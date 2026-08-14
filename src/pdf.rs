use std::io::BufWriter;

use printpdf::path::PaintMode;
use printpdf::*;

use crate::error::{NigelError, Result};
use crate::fmt::money;
use crate::invoicing::document::{
    address_lines, email_line, meta_rows, payment_lines, row_is_shaded, terms_block_text,
    CompanyBlock, DocumentColor, Logo, MoneySummary, BORDER_GRAY, LOGO_HEIGHT_FRACTION,
    LOGO_WIDTH_FRACTION, ROW_SHADE,
};
use crate::models::{Client, Invoice, InvoiceLineItem};
use crate::reports::*;

// US Letter dimensions (mm)
const PAGE_W: f32 = 215.9;
const PAGE_H: f32 = 279.4;
const MARGIN_TOP: f32 = 25.4;
const MARGIN_BOTTOM: f32 = 25.4;
const MARGIN_LEFT: f32 = 19.05;
const MARGIN_RIGHT: f32 = 19.05;
const ROW_H: f32 = 5.0;
const COL_PAD: f32 = 6.0;

/// The air above and below a line-item row's type, inside its rules.
///
/// The item table is the one block a reader scans rather than reads, and type
/// sitting hard against a rule is what made it read as cramped. Every metric
/// that describes a row — the zebra band, the rule under it, the column
/// dividers' extent — is derived from the same number, so they cannot come
/// apart.
const CELL_PAD_Y: f32 = 1.8;
const FONT_SIZE: f32 = 10.0;
const TITLE_SIZE: f32 = 16.0;
const SUBTITLE_SIZE: f32 = 10.0;

fn approx_text_width(text: &str, size: f32) -> f32 {
    text.len() as f32 * size * 0.18
}

fn wrap_text(text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let test = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };

        if approx_text_width(&test, font_size) <= max_width {
            current = test;
        } else {
            if !current.is_empty() {
                lines.push(current);
            }
            current = word.to_string();
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Right,
}

struct Col {
    width: f32,
    align: Align,
}

/// Single full-width column for note lines that span the printable area.
const NOTE_COLS: &[Col] = &[Col {
    width: PAGE_W - MARGIN_LEFT - MARGIN_RIGHT,
    align: Align::Left,
}];

struct PdfWriter {
    doc: PdfDocumentReference,
    font: IndirectFontRef,
    font_bold: IndirectFontRef,
    current_page: PdfPageIndex,
    current_layer: PdfLayerIndex,
    y: f32,
    /// How many pages have been started. The one thing that tells a caller
    /// whether a block it was drawing crossed a page break: `y` resets on a new
    /// page, so it can be *lower* after a break than before one and cannot be
    /// compared across it.
    page_no: usize,
}

impl PdfWriter {
    fn new(title: &str) -> Result<Self> {
        let (doc, page, layer) = PdfDocument::new(title, Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        let font = doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| NigelError::Pdf(format!("{e:?}")))?;
        let font_bold = doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| NigelError::Pdf(format!("{e:?}")))?;
        Ok(Self {
            doc,
            font,
            font_bold,
            current_page: page,
            current_layer: layer,
            y: MARGIN_TOP,
            page_no: 0,
        })
    }

    fn pdf_y(&self) -> f32 {
        PAGE_H - self.y
    }

    fn new_page(&mut self) {
        let (page, layer) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer");
        self.current_page = page;
        self.current_layer = layer;
        self.y = MARGIN_TOP;
        self.page_no += 1;
    }

    fn ensure_space(&mut self, needed: f32) {
        if self.y + needed > PAGE_H - MARGIN_BOTTOM {
            self.new_page();
        }
    }

    fn text(&self, s: &str, x: f32, size: f32, bold: bool) {
        let font = if bold {
            self.font_bold.clone()
        } else {
            self.font.clone()
        };
        let layer = self
            .doc
            .get_page(self.current_page)
            .get_layer(self.current_layer);
        layer.use_text(s, size, Mm(x), Mm(self.pdf_y()), &font);
    }

    /// Right-aligned text ending at `right_edge`, for the blocks that are not
    /// table cells: the metadata labels and the Amount Due figures.
    fn text_right(&self, s: &str, right_edge: f32, size: f32, bold: bool) {
        self.text(s, right_edge - approx_text_width(s, size), size, bold);
    }

    /// The layer this writer is drawing on.
    fn layer(&self) -> PdfLayerReference {
        self.doc
            .get_page(self.current_page)
            .get_layer(self.current_layer)
    }

    /// A filled band the width of the item table, for the zebra striping.
    ///
    /// The fill colour is restored to black afterwards, because `use_text`
    /// inherits it: printpdf's text operator sets the font and the cursor and
    /// nothing else, so a row drawn after an unrestored grey fill would be grey
    /// type on a grey ground.
    fn fill_band(&self, x1: f32, y_from: f32, x2: f32, y_to: f32, color: DocumentColor) {
        let (r, g, b) = color.unit_rgb();
        let layer = self.layer();
        layer.set_fill_color(Color::Rgb(Rgb::new(r, g, b, None)));
        layer.add_rect(
            Rect::new(Mm(x1), Mm(PAGE_H - y_to), Mm(x2), Mm(PAGE_H - y_from))
                .with_mode(PaintMode::Fill),
        );
        layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
    }

    /// A vertical rule, for the party blocks and the item table's dividers.
    /// `y_from`/`y_to` are this writer's downward `y`, not PDF coordinates.
    fn vline(&self, x: f32, y_from: f32, y_to: f32) {
        let layer = self.layer();
        self.use_border_color(&layer);
        layer.set_outline_thickness(0.5);
        layer.add_line(Line {
            points: vec![
                (Point::new(Mm(x), Mm(PAGE_H - y_from)), false),
                (Point::new(Mm(x), Mm(PAGE_H - y_to)), false),
            ],
            is_closed: false,
        });
    }

    /// Every rule on this document is the one grey both documents share.
    fn use_border_color(&self, layer: &PdfLayerReference) {
        let (r, g, b) = BORDER_GRAY.unit_rgb();
        layer.set_outline_color(Color::Rgb(Rgb::new(r, g, b, None)));
    }

    fn hline(&self, x1: f32, x2: f32) {
        let layer = self.layer();
        self.use_border_color(&layer);
        layer.set_outline_thickness(0.5);
        let line = Line {
            points: vec![
                (Point::new(Mm(x1), Mm(self.pdf_y())), false),
                (Point::new(Mm(x2), Mm(self.pdf_y())), false),
            ],
            is_closed: false,
        };
        layer.add_line(line);
    }

    fn header(&mut self, title: &str, company: &str, date_range: &str) {
        self.text(title, MARGIN_LEFT, TITLE_SIZE, true);
        self.y += 7.0;
        if !company.is_empty() {
            self.text(company, MARGIN_LEFT, SUBTITLE_SIZE, false);
            self.y += 5.0;
        }
        self.text(date_range, MARGIN_LEFT, SUBTITLE_SIZE, false);
        self.y += 5.0;
        let ts = chrono::Local::now()
            .format("Generated %Y-%m-%d %H:%M")
            .to_string();
        self.text(&ts, MARGIN_LEFT, 8.0, false);
        self.y += 5.0;
        self.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
        self.y += 5.0;
    }

    fn table_header(&mut self, cols: &[Col], headers: &[&str]) {
        self.ensure_space(ROW_H * 2.0);
        let mut x = MARGIN_LEFT;
        for (i, col) in cols.iter().enumerate() {
            if i < headers.len() {
                match col.align {
                    Align::Left => self.text(headers[i], x, FONT_SIZE, true),
                    Align::Right => {
                        let tw = approx_text_width(headers[i], FONT_SIZE);
                        self.text(headers[i], x + col.width - COL_PAD - tw, FONT_SIZE, true);
                    }
                }
            }
            x += col.width;
        }
        self.y += 3.5;
        self.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
        self.y += 5.0;
    }

    fn table_row(&mut self, cols: &[Col], values: &[&str], bold: bool) {
        self.ensure_space(ROW_H);
        let mut x = MARGIN_LEFT;
        for (i, col) in cols.iter().enumerate() {
            if i < values.len() {
                match col.align {
                    Align::Left => self.text(values[i], x, FONT_SIZE, bold),
                    Align::Right => {
                        let tw = approx_text_width(values[i], FONT_SIZE);
                        self.text(values[i], x + col.width - COL_PAD - tw, FONT_SIZE, bold);
                    }
                }
            }
            x += col.width;
        }
        self.y += ROW_H;
    }

    /// One line-item row: its zebra band, its cells, and the rule under it.
    ///
    /// The order matters and the page break matters. `ensure_space` runs
    /// **before** anything is drawn, so a row that does not fit starts the new
    /// page and then paints its band, its text and its rule there — which is
    /// what makes the striping and the grid carry on correctly across a break
    /// rather than stranding a band on the page the row left.
    fn item_row(&mut self, cols: &[Col], values: &[&str], shaded: bool) {
        let wrapped = Self::wrap_cells(cols, values, FONT_SIZE);
        let max_lines = wrapped.iter().map(|w| w.len()).max().unwrap_or(1);
        let row_height = max_lines as f32 * ROW_H + 2.0 * CELL_PAD_Y;
        self.ensure_space(row_height);

        // The band is the whole row — padding included — so the striping covers
        // what the rules enclose rather than the type alone.
        let top = self.y - 3.5;
        if shaded {
            self.fill_band(
                MARGIN_LEFT,
                top,
                PAGE_W - MARGIN_RIGHT,
                top + row_height,
                ROW_SHADE,
            );
        }
        self.y += CELL_PAD_Y;
        self.draw_cells(cols, &wrapped, max_lines, false, FONT_SIZE);
        self.y += CELL_PAD_Y;

        let saved = self.y;
        self.y = top + row_height;
        self.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
        self.y = saved;
    }

    fn wrap_cells(cols: &[Col], values: &[&str], font_size: f32) -> Vec<Vec<String>> {
        cols.iter()
            .enumerate()
            .map(|(i, col)| {
                if i < values.len() && !values[i].is_empty() {
                    wrap_text(values[i], col.width - COL_PAD, font_size)
                } else {
                    vec![String::new()]
                }
            })
            .collect()
    }

    fn draw_cells(
        &mut self,
        cols: &[Col],
        wrapped: &[Vec<String>],
        max_lines: usize,
        bold: bool,
        font_size: f32,
    ) {
        for line_idx in 0..max_lines {
            let mut x = MARGIN_LEFT;
            for (col_idx, col) in cols.iter().enumerate() {
                if let Some(text) = wrapped.get(col_idx).and_then(|c| c.get(line_idx)) {
                    if !text.is_empty() {
                        match col.align {
                            Align::Left => self.text(text, x, font_size, bold),
                            Align::Right => {
                                let tw = approx_text_width(text, font_size);
                                self.text(text, x + col.width - COL_PAD - tw, font_size, bold);
                            }
                        }
                    }
                }
                x += col.width;
            }
            self.y += ROW_H;
        }
    }

    fn table_row_wrapped(&mut self, cols: &[Col], values: &[&str], bold: bool, font_size: f32) {
        // Wrap each cell's text to fit its column width minus padding
        let wrapped: Vec<Vec<String>> = cols
            .iter()
            .enumerate()
            .map(|(i, col)| {
                if i < values.len() && !values[i].is_empty() {
                    wrap_text(values[i], col.width - COL_PAD, font_size)
                } else {
                    vec![String::new()]
                }
            })
            .collect();

        let max_lines = wrapped.iter().map(|w| w.len()).max().unwrap_or(1);
        let row_height = max_lines as f32 * ROW_H;
        self.ensure_space(row_height);

        for line_idx in 0..max_lines {
            let mut x = MARGIN_LEFT;
            for (col_idx, col) in cols.iter().enumerate() {
                if col_idx < wrapped.len() {
                    if let Some(text) = wrapped[col_idx].get(line_idx) {
                        if !text.is_empty() {
                            match col.align {
                                Align::Left => self.text(text, x, font_size, bold),
                                Align::Right => {
                                    let tw = approx_text_width(text, font_size);
                                    self.text(text, x + col.width - COL_PAD - tw, font_size, bold);
                                }
                            }
                        }
                    }
                }
                x += col.width;
            }
            self.y += ROW_H;
        }
    }

    fn section_label(&mut self, label: &str) {
        self.ensure_space(ROW_H);
        self.text(label, MARGIN_LEFT, FONT_SIZE, true);
        self.y += ROW_H;
    }

    fn blank_row(&mut self) {
        self.y += ROW_H;
    }

    fn separator(&mut self) {
        self.y -= 1.0;
        self.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
        self.y += 5.5;
    }

    fn into_bytes(self) -> Result<Vec<u8>> {
        let mut buf = BufWriter::new(Vec::new());
        self.doc
            .save(&mut buf)
            .map_err(|e| NigelError::Pdf(format!("{e:?}")))?;
        buf.into_inner().map_err(|e| NigelError::Pdf(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Render functions
// ---------------------------------------------------------------------------

pub fn render_pnl(report: &PnlReport, company: &str, date_range: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Profit & Loss")?;
    pdf.header("Profit & Loss", company, date_range);

    let cols = &[
        Col {
            width: 130.0,
            align: Align::Left,
        },
        Col {
            width: 47.8,
            align: Align::Right,
        },
    ];
    pdf.table_header(cols, &["Category", "Amount"]);

    if !report.income.is_empty() {
        pdf.section_label("INCOME");
        for item in &report.income {
            let amt = money(item.total);
            pdf.table_row(cols, &[&item.name, &amt], false);
        }
        let total = money(report.total_income);
        pdf.table_row(cols, &["Total Income", &total], true);
        pdf.blank_row();
    }

    if !report.expenses.is_empty() {
        pdf.section_label("EXPENSES");
        for item in &report.expenses {
            let amt = money(item.total.abs());
            pdf.table_row(cols, &[&item.name, &amt], false);
        }
        let total = money(report.total_expenses.abs());
        pdf.table_row(cols, &["Total Expenses", &total], true);
        pdf.blank_row();
    }

    pdf.separator();
    let label = if report.net >= 0.0 {
        "NET INCOME"
    } else {
        "NET LOSS"
    };
    let net = money(report.net);
    pdf.table_row(cols, &[label, &net], true);

    pdf.into_bytes()
}

pub fn render_expenses(
    report: &ExpenseBreakdown,
    company: &str,
    date_range: &str,
) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Expense Breakdown")?;
    pdf.header("Expense Breakdown", company, date_range);

    let cols = &[
        Col {
            width: 90.0,
            align: Align::Left,
        },
        Col {
            width: 40.0,
            align: Align::Right,
        },
        Col {
            width: 27.8,
            align: Align::Right,
        },
        Col {
            width: 20.0,
            align: Align::Right,
        },
    ];
    pdf.table_header(cols, &["Category", "Amount", "%", "Count"]);

    for item in &report.categories {
        let amt = money(item.total.abs());
        let pct = format!("{:.1}%", item.pct);
        let cnt = item.count.to_string();
        pdf.table_row(cols, &[&item.name, &amt, &pct, &cnt], false);
    }
    let total = money(report.total.abs());
    pdf.separator();
    pdf.table_row(cols, &["Total", &total, "", ""], true);

    if !report.top_vendors.is_empty() {
        pdf.blank_row();
        pdf.section_label("Top Vendors");
        let vcols = &[
            Col {
                width: 90.0,
                align: Align::Left,
            },
            Col {
                width: 40.0,
                align: Align::Right,
            },
            Col {
                width: 47.8,
                align: Align::Right,
            },
        ];
        pdf.table_header(vcols, &["Vendor", "Amount", "Count"]);
        for v in &report.top_vendors {
            let amt = money(v.total.abs());
            let cnt = v.count.to_string();
            pdf.table_row(vcols, &[&v.vendor, &amt, &cnt], false);
        }
    }

    pdf.into_bytes()
}

pub fn render_tax(report: &TaxSummary, company: &str, date_range: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Tax Summary")?;
    pdf.header("Tax Summary", company, date_range);

    let cols = &[
        Col {
            width: 70.0,
            align: Align::Left,
        },
        Col {
            width: 40.0,
            align: Align::Left,
        },
        Col {
            width: 30.0,
            align: Align::Left,
        },
        Col {
            width: 37.8,
            align: Align::Right,
        },
    ];
    pdf.table_header(cols, &["Category", "Tax Line", "Type", "Amount"]);

    for item in &report.line_items {
        let tl = item.tax_line.as_deref().unwrap_or("");
        let amt = money(item.total.abs());
        pdf.table_row(cols, &[&item.name, tl, &item.category_type, &amt], false);
    }

    pdf.into_bytes()
}

pub fn render_cashflow(
    report: &CashflowReport,
    company: &str,
    date_range: &str,
) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Cash Flow")?;
    pdf.header("Cash Flow", company, date_range);

    let cols = &[
        Col {
            width: 35.0,
            align: Align::Left,
        },
        Col {
            width: 37.0,
            align: Align::Right,
        },
        Col {
            width: 37.0,
            align: Align::Right,
        },
        Col {
            width: 37.0,
            align: Align::Right,
        },
        Col {
            width: 31.8,
            align: Align::Right,
        },
    ];
    pdf.table_header(cols, &["Month", "Inflows", "Outflows", "Net", "Running"]);

    for m in &report.months {
        let inf = money(m.inflows);
        let out = money(m.outflows.abs());
        let net = money(m.net);
        let run = money(m.running_balance);
        pdf.table_row(cols, &[&m.month, &inf, &out, &net, &run], false);
    }

    pdf.into_bytes()
}

pub fn render_register(
    report: &RegisterReport,
    company: &str,
    date_range: &str,
) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Transaction Register")?;
    pdf.header(
        "Transaction Register",
        company,
        &format!("{date_range} — {} transactions", report.rows.len()),
    );

    let cols = &[
        Col {
            width: 20.0,
            align: Align::Left,
        },
        Col {
            width: 62.0,
            align: Align::Left,
        },
        Col {
            width: 22.0,
            align: Align::Right,
        },
        Col {
            width: 42.0,
            align: Align::Left,
        },
        Col {
            width: 31.8,
            align: Align::Left,
        },
    ];
    let font_size = 8.0;
    pdf.table_header(
        cols,
        &["Date", "Description", "Amount", "Category", "Account"],
    );

    for r in &report.rows {
        let amt = money(r.amount);
        let cat = r.category.as_deref().unwrap_or("—");
        pdf.table_row_wrapped(
            cols,
            &[&r.date, &r.description, &amt, cat, &r.account_name],
            false,
            font_size,
        );
    }

    pdf.separator();
    let total = money(report.total);
    let count_label = format!("{} transactions", report.rows.len());
    pdf.table_row(cols, &[&count_label, "", &total, "", ""], true);

    pdf.into_bytes()
}

pub fn render_flagged(rows: &[FlaggedTransaction], company: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Flagged Transactions")?;
    pdf.header(
        "Flagged Transactions",
        company,
        &format!("{} items", rows.len()),
    );

    let cols = &[
        Col {
            width: 15.0,
            align: Align::Left,
        },
        Col {
            width: 27.0,
            align: Align::Left,
        },
        Col {
            width: 80.0,
            align: Align::Left,
        },
        Col {
            width: 30.0,
            align: Align::Right,
        },
        Col {
            width: 25.8,
            align: Align::Left,
        },
    ];
    pdf.table_header(cols, &["ID", "Date", "Description", "Amount", "Account"]);

    for r in rows {
        let id = r.id.to_string();
        let amt = money(r.amount.abs());
        pdf.table_row(
            cols,
            &[&id, &r.date, &r.description, &amt, &r.account_name],
            false,
        );
    }

    pdf.into_bytes()
}

pub fn render_balance(report: &BalanceReport, company: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("Cash Position")?;
    pdf.header("Cash Position", company, "As of today");

    let cols = &[
        Col {
            width: 80.0,
            align: Align::Left,
        },
        Col {
            width: 50.0,
            align: Align::Left,
        },
        Col {
            width: 47.8,
            align: Align::Right,
        },
    ];
    pdf.table_header(cols, &["Account", "Type", "Balance"]);

    for a in &report.accounts {
        let bal = money(a.balance);
        pdf.table_row(cols, &[&a.name, &a.account_type, &bal], false);
    }

    pdf.separator();
    let total = money(report.total);
    pdf.table_row(cols, &["Total", "", &total], true);

    pdf.blank_row();
    let ytd = money(report.ytd_net_income);
    let ytd_label = format!("YTD Net Income: {ytd}");
    pdf.text(&ytd_label, MARGIN_LEFT, FONT_SIZE, false);

    pdf.into_bytes()
}

pub fn render_aging(
    report: &crate::invoicing::invoices::AgingReport,
    company: &str,
) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("A/R Aging")?;
    pdf.header("A/R Aging", company, &format!("As of {}", report.as_of));

    let summary_cols = &[
        Col {
            width: 80.0,
            align: Align::Left,
        },
        Col {
            width: 50.0,
            align: Align::Right,
        },
        Col {
            width: 47.8,
            align: Align::Right,
        },
    ];
    pdf.section_label("Summary");
    pdf.table_header(summary_cols, &["Bucket", "Invoices", "Amount"]);
    for b in &report.buckets {
        let count = b.count.to_string();
        let total = money(b.total);
        pdf.table_row(summary_cols, &[b.label, &count, &total], false);
    }
    pdf.separator();
    let count = report
        .buckets
        .iter()
        .map(|b| b.count)
        .sum::<usize>()
        .to_string();
    let outstanding = money(report.outstanding);
    pdf.table_row(
        summary_cols,
        &["Total Outstanding", &count, &outstanding],
        true,
    );

    pdf.blank_row();
    pdf.section_label("Open Invoices");

    if report.invoices.is_empty() {
        pdf.text("No open invoices.", MARGIN_LEFT, FONT_SIZE, false);
        return pdf.into_bytes();
    }

    let cols = &[
        Col {
            width: 25.0,
            align: Align::Left,
        },
        Col {
            width: 62.8,
            align: Align::Left,
        },
        Col {
            width: 30.0,
            align: Align::Left,
        },
        Col {
            width: 20.0,
            align: Align::Right,
        },
        Col {
            width: 40.0,
            align: Align::Right,
        },
    ];
    pdf.table_header(cols, &["Invoice", "Client", "Due", "Days", "Balance"]);
    for i in &report.invoices {
        let number = format!("#{}", i.number);
        let days = if i.days_past_due > 0 {
            i.days_past_due.to_string()
        } else {
            "\u{2014}".to_string()
        };
        let balance = money(i.balance);
        pdf.table_row(
            cols,
            &[&number, &i.client, &i.due_date, &days, &balance],
            false,
        );
    }

    pdf.into_bytes()
}

pub fn render_k1(report: &K1PrepReport, company: &str, date_range: &str) -> Result<Vec<u8>> {
    let mut pdf = PdfWriter::new("K-1 Preparation Worksheet")?;
    pdf.header(
        "K-1 Preparation Worksheet (Form 1120-S)",
        company,
        date_range,
    );

    // Income Summary
    let summary_cols = &[
        Col {
            width: 130.0,
            align: Align::Left,
        },
        Col {
            width: 47.8,
            align: Align::Right,
        },
    ];
    pdf.section_label("Income Summary");
    pdf.table_header(summary_cols, &["Item", "Amount"]);
    let gr = money(report.gross_receipts);
    pdf.table_row(summary_cols, &["Gross Receipts", &gr], false);
    let cogs = money(report.cogs);
    pdf.table_row(summary_cols, &["Cost of Goods Sold", &cogs], false);
    let gp = money(report.gross_profit);
    pdf.table_row(summary_cols, &["Gross Profit", &gp], false);
    let oi = money(report.other_income);
    pdf.table_row(summary_cols, &["Other Income", &oi], false);
    let td = money(report.total_deductions);
    pdf.table_row(summary_cols, &["Total Deductions", &td], false);
    pdf.separator();
    let label = if report.ordinary_business_income >= 0.0 {
        "Ordinary Business Income"
    } else {
        "Ordinary Business Loss"
    };
    let obi = money(report.ordinary_business_income);
    pdf.table_row(summary_cols, &[label, &obi], true);

    if !report.auto_mapped.is_empty() {
        let note = format!(
            "(auto) income mapped to gross receipts: {}",
            report.auto_mapped.join(", ")
        );
        pdf.table_row_wrapped(NOTE_COLS, &[&note], false, FONT_SIZE);
    }
    pdf.blank_row();

    // Deductions by Line
    if !report.deduction_lines.is_empty() {
        let ded_cols = &[
            Col {
                width: 30.0,
                align: Align::Left,
            },
            Col {
                width: 100.0,
                align: Align::Left,
            },
            Col {
                width: 47.8,
                align: Align::Right,
            },
        ];
        pdf.section_label("Deductions by Line");
        pdf.table_header(ded_cols, &["Line", "Category", "Amount"]);
        for item in &report.deduction_lines {
            let amt = money(item.total);
            pdf.table_row(
                ded_cols,
                &[&item.form_line, &item.category_name, &amt],
                false,
            );
        }
        pdf.blank_row();
    }

    // Schedule K Items
    if !report.schedule_k_items.is_empty() {
        let sk_cols = &[
            Col {
                width: 30.0,
                align: Align::Left,
            },
            Col {
                width: 100.0,
                align: Align::Left,
            },
            Col {
                width: 47.8,
                align: Align::Right,
            },
        ];
        pdf.section_label("Schedule K");
        pdf.table_header(sk_cols, &["Line", "Item", "Amount"]);
        for item in &report.schedule_k_items {
            let amt = money(item.total.abs());
            pdf.table_row(
                sk_cols,
                &[&item.form_line, &item.category_name, &amt],
                false,
            );
        }
        pdf.blank_row();
    }

    // Line 19 Other Deductions detail
    if !report.other_deductions.is_empty() {
        let od_cols = &[
            Col {
                width: 80.0,
                align: Align::Left,
            },
            Col {
                width: 48.9,
                align: Align::Right,
            },
            Col {
                width: 48.9,
                align: Align::Right,
            },
        ];
        pdf.section_label("Line 19 — Other Deductions");
        pdf.table_header(od_cols, &["Category", "Full Amount", "Deductible"]);
        for item in &report.other_deductions {
            let label = if item.deductible < item.total {
                format!("{} (50%)", item.category_name)
            } else {
                item.category_name.clone()
            };
            let full = money(item.total);
            let ded = money(item.deductible);
            pdf.table_row(od_cols, &[&label, &full, &ded], false);
        }
        pdf.separator();
        let odt = money(report.other_deductions_total);
        pdf.table_row(od_cols, &["Total Other Deductions", "", &odt], true);
    }

    // Validation notes
    if report.validation.uncategorized_count > 0 {
        pdf.blank_row();
        let warning = format!(
            "Warning: {} uncategorized transactions",
            report.validation.uncategorized_count
        );
        pdf.text(&warning, MARGIN_LEFT, FONT_SIZE, true);
        pdf.y += ROW_H;
    }

    // Needs mapping
    if !report.unmapped.is_empty() {
        pdf.blank_row();
        let um_cols = &[
            Col {
                width: 130.0,
                align: Align::Left,
            },
            Col {
                width: 47.8,
                align: Align::Right,
            },
        ];
        pdf.section_label("Needs mapping");
        pdf.table_row_wrapped(
            NOTE_COLS,
            &["These categories have activity but no form_line; they are excluded from the totals above."],
            false,
            FONT_SIZE,
        );
        pdf.table_header(um_cols, &["Category", "Amount"]);
        for item in &report.unmapped {
            let amt = money(item.total);
            pdf.table_row(um_cols, &[&item.category_name, &amt], false);
        }
    }

    pdf.into_bytes()
}

/// What a PDF viewer shows in its window title and what a saved file is called
/// by default, so the operator's name belongs in it when there is one.
fn document_title(title: &str, company: &str) -> String {
    if company.is_empty() {
        title.to_string()
    } else {
        format!("{company} - {title}")
    }
}

/// The box a logo is fitted into, top-left of the page.
///
/// Both caps are the shared fractions of this document's printable width, so
/// the mark reads at the same size here as it does on the page — a masthead
/// rather than a banner. Whichever dimension binds first is the aspect ratio's
/// business, and either way the box stays clear of the From block.
const PRINTABLE_W: f32 = PAGE_W - MARGIN_LEFT - MARGIN_RIGHT;
const LOGO_MAX_W: f32 = PRINTABLE_W * LOGO_WIDTH_FRACTION;
const LOGO_MAX_H: f32 = PRINTABLE_W * LOGO_HEIGHT_FRACTION;

/// Where the two right-hand party blocks — From, and Invoice For — start.
const PARTY_LABEL_X: f32 = 110.0;
const PARTY_RULE_X: f32 = 126.0;
const PARTY_TEXT_X: f32 = 129.0;

/// How wide a party block's lines may be before they wrap.
const PARTY_WIDTH: f32 = PAGE_W - MARGIN_RIGHT - PARTY_TEXT_X;

/// The metadata column's value column, beside its labels at `MARGIN_LEFT`.
const META_VALUE_X: f32 = MARGIN_LEFT + 30.0;

/// How wide a metadata value may be before it is cut: up to the party column,
/// which starts beside it. `due_value` can put a whole terms sentence here.
const META_VALUE_WIDTH: f32 = PARTY_LABEL_X - META_VALUE_X - COL_PAD;

/// The wordmark's size when there is room for it, and the smallest it may shrink
/// to before the name is cut instead. Below the body size it stops reading as a
/// letterhead and starts reading as a mistake.
const WORDMARK_SIZE: f32 = 22.0;
const WORDMARK_MIN_SIZE: f32 = FONT_SIZE;

/// How wide the wordmark may be: the whole left half of the letterhead band, up
/// to where the From block's label starts.
const WORDMARK_WIDTH: f32 = PARTY_LABEL_X - MARGIN_LEFT;

/// What a value cut to fit ends with, the same marker `document::address_lines`
/// uses when it clamps an address.
const TRUNCATED: &str = "...";

const LINE_H: f32 = 5.0;

/// `text` cut down until it fits `max_width` at `size`, ending in `TRUNCATED`
/// when anything was dropped. A value that already fits comes back untouched.
fn truncate_to_width(text: &str, max_width: f32, size: f32) -> String {
    if approx_text_width(text, size) <= max_width {
        return text.to_string();
    }
    let mut kept = String::new();
    for ch in text.chars() {
        let mut candidate = kept.clone();
        candidate.push(ch);
        if approx_text_width(&(candidate.clone() + TRUNCATED), size) > max_width {
            break;
        }
        kept = candidate;
    }
    kept.push_str(TRUNCATED);
    kept
}

impl PdfWriter {
    /// The company name where a logo would go: bold at the left margin, in the
    /// space left of the From block. What every document without a usable image
    /// is headed by.
    ///
    /// A name too wide for that space is **shrunk first and cut second**. A
    /// business name is somebody's own, so making it smaller keeps all of it;
    /// cutting only happens once shrinking would take the wordmark below the
    /// body size, at which point it has stopped being a letterhead anyway.
    /// Either way it never reaches the From block, which it would otherwise
    /// print straight through.
    fn wordmark(&mut self, name: &str) {
        if name.is_empty() {
            return;
        }
        let mut size = WORDMARK_SIZE;
        while size > WORDMARK_MIN_SIZE && approx_text_width(name, size) > WORDMARK_WIDTH {
            size -= 0.5;
        }
        let drawn = truncate_to_width(name, WORDMARK_WIDTH, size);
        // The baseline sits a line down from the band's top, so the wordmark
        // occupies the band rather than hanging above it.
        self.y += 7.0;
        self.text(&drawn, MARGIN_LEFT, size, true);
        self.y += 3.0;
    }

    /// Draw the logo top-left, fitted to `LOGO_MAX_W` × `LOGO_MAX_H` with its
    /// aspect ratio preserved. Answers whether it drew: every refusal is the
    /// caller's cue to draw the wordmark instead, and none of them is an error.
    #[cfg(feature = "pdf")]
    fn logo(&mut self, logo: &Logo) -> bool {
        let Some(image) = prepare_logo(&logo.bytes) else {
            return false;
        };
        let (px_w, px_h) = (logo_dimensions(&image).0, logo_dimensions(&image).1);
        if px_w == 0 || px_h == 0 {
            return false;
        }

        // Fill the box in whichever dimension binds first, so a wide wordmark
        // is 60 mm across and short, and a tall mark is 16 mm high and narrow.
        let aspect = px_w as f32 / px_h as f32;
        let (draw_w, draw_h) = if LOGO_MAX_H * aspect <= LOGO_MAX_W {
            (LOGO_MAX_H * aspect, LOGO_MAX_H)
        } else {
            (LOGO_MAX_W, LOGO_MAX_W / aspect)
        };

        // printpdf lays an image out at `dpi`, then applies the scale. Naming
        // the dpi rather than taking the default keeps the arithmetic here the
        // same arithmetic it does.
        const DPI: f32 = 300.0;
        let natural_w = px_w as f32 / DPI * 25.4;
        let natural_h = px_h as f32 / DPI * 25.4;

        let layer = self
            .doc
            .get_page(self.current_page)
            .get_layer(self.current_layer);
        Image::from_dynamic_image(&image).add_to_layer(
            layer,
            ImageTransform {
                translate_x: Some(Mm(MARGIN_LEFT)),
                // PDF coordinates, and an image is placed by its bottom edge.
                translate_y: Some(Mm(PAGE_H - self.y - draw_h)),
                rotate: None,
                scale_x: Some(draw_w / natural_w),
                scale_y: Some(draw_h / natural_h),
                dpi: Some(DPI),
            },
        );
        self.y += draw_h;
        true
    }

    /// A ruled party block — a label, a vertical rule, and the lines beside it.
    /// Draws nothing at all when there is nothing to say.
    ///
    /// Every line the block carries arrives in `lines`, so the rule is drawn
    /// once, last, over everything it brackets. A caller that drew one more line
    /// afterwards would leave that line outside the rule and outside the
    /// emptiness test both.
    ///
    /// Lines wrap inside the column. This one is `PARTY_WIDTH` wide, `self.text`
    /// does not wrap, and a client's postal address running off the right edge
    /// of the page is a document that has lost part of where it is going.
    fn party_block(&mut self, label: &str, name: &str, lines: &[&str], bold_name: bool) {
        if name.is_empty() && lines.is_empty() {
            return;
        }
        let top = self.y - 3.5;
        self.text(label, PARTY_LABEL_X, 7.0, false);
        if !name.is_empty() {
            for wrapped in wrap_text(name, PARTY_WIDTH, FONT_SIZE) {
                self.text(&wrapped, PARTY_TEXT_X, FONT_SIZE, bold_name);
                self.y += LINE_H;
            }
        }
        for line in lines {
            for wrapped in wrap_text(line, PARTY_WIDTH, FONT_SIZE) {
                self.text(&wrapped, PARTY_TEXT_X, FONT_SIZE, false);
                self.y += LINE_H;
            }
        }
        self.vline(PARTY_RULE_X, top, self.y - 3.5);
    }
}

/// The image printpdf is handed, and the only thing that builds one.
///
/// Always `Rgb8`: any alpha is composited onto white first, because printpdf
/// 0.7's soft-mask path sizes a transparent image's mask from the image's
/// *width* (`xobject.rs`, `impl From<ImageXObject> for lopdf::Stream`), so a
/// wide transparent wordmark embeds wrong. White because a PDF page is white,
/// and compositing onto the surface the image will sit on is the only choice
/// that is not an invention about someone's brand. `None` for anything that
/// will not decode — a logo is decoration on a document about money.
/// Whether this logo will actually decode into something embeddable.
///
/// The half of "is this logo usable" that needs a decoder, which is why it lives
/// here and not in `document.rs`: `image` arrives with the `pdf` feature and
/// exists nowhere else. `render_invoice` calls it once, above both renderers, so
/// a file the page would happily put in an `<img>` and this document could not
/// embed is dropped from both rather than shown by one.
#[cfg(feature = "pdf")]
pub fn logo_is_embeddable(logo: &Logo) -> bool {
    prepare_logo(&logo.bytes).is_some()
}

#[cfg(feature = "pdf")]
fn prepare_logo(bytes: &[u8]) -> Option<image_crate::DynamicImage> {
    let decoded = image_crate::load_from_memory(bytes).ok()?;
    let source = decoded.to_rgba8();
    let mut flattened = image_crate::RgbImage::new(source.width(), source.height());
    for (x, y, pixel) in source.enumerate_pixels() {
        let alpha = pixel[3] as u32;
        let over =
            |channel: u8| ((channel as u32 * alpha + 255 * (255 - alpha)) / 255).min(255) as u8;
        flattened.put_pixel(
            x,
            y,
            image_crate::Rgb([over(pixel[0]), over(pixel[1]), over(pixel[2])]),
        );
    }
    Some(image_crate::DynamicImage::ImageRgb8(flattened))
}

#[cfg(feature = "pdf")]
fn logo_dimensions(image: &image_crate::DynamicImage) -> (u32, u32) {
    use image_crate::GenericImageView as _;
    image.dimensions()
}

/// The invoice as the client's email attachment carries it.
///
/// `company` is the whole From block, decided by `document::company_block` so
/// that the page cannot draw a different one. A logo is drawn top-left when
/// there is a usable one and the company name is drawn as a wordmark when there
/// is not — a logo problem degrades this document, never fails it.
///
/// It deliberately carries **no payment link and no URL at all**. An emailed
/// attachment cannot be recalled or republished, so a live charge link in it
/// would survive the settlement it was created for — the same reasoning that
/// makes void deactivate links — and a tokenized page address printed as
/// unclickable text is sixty characters of noise beside the figure that
/// matters. Paying online is the published page's job, and the email carries
/// its link.
pub fn render_invoice_pdf(
    invoice: &Invoice,
    client: &Client,
    company: &CompanyBlock<'_>,
    logo: Option<&Logo>,
    items: &[InvoiceLineItem],
    summary: &MoneySummary,
    payment_instructions: &str,
) -> Result<Vec<u8>> {
    let title = format!("Invoice #{}", invoice.number);
    let mut pdf = PdfWriter::new(&document_title(&title, company.name))?;

    // --- letterhead: the mark on the left, the From block on the right ------
    let band_top = pdf.y;
    let drawn = match logo {
        Some(logo) => pdf.logo(logo),
        None => false,
    };
    if !drawn {
        pdf.wordmark(company.name);
    }
    let left_bottom = pdf.y;

    pdf.y = band_top;
    // The phone is one of the block's lines, not something drawn after it: the
    // block decides from all of them whether there is anything to draw, and its
    // rule brackets all of them.
    let phone_line = company.phone.map(|phone| format!("ph. {phone}"));
    let mut from_lines: Vec<&str> = company.address.clone();
    from_lines.extend(phone_line.as_deref());
    pdf.party_block("From", company.name, &from_lines, true);
    // No title line. The letterhead is this document's masthead and the
    // metadata band carries the identifier, so drawing "Invoice #1248" here
    // would say the same thing twice — once as a heading and once as the row a
    // client actually quotes back. `title` stays: it is the document's Info
    // title, which is what a viewer puts in its window and a browser suggests
    // as a filename, and that is file metadata rather than visible layout.
    pdf.y = pdf.y.max(left_bottom) + 12.0;

    // --- the metadata column, and who the invoice is for --------------------
    let band_top = pdf.y;
    for row in meta_rows(invoice) {
        pdf.text(row.label, MARGIN_LEFT, FONT_SIZE, false);
        // Cut rather than wrapped: these are label/value pairs on one line
        // each, and `due_value` can fold a whole terms sentence into one of
        // them. Unbounded it prints straight through the party column beside it.
        let value = truncate_to_width(&row.value, META_VALUE_WIDTH, FONT_SIZE);
        pdf.text(&value, META_VALUE_X, FONT_SIZE, row.emphasis);
        pdf.y += LINE_H;
    }
    let left_bottom = pdf.y;

    pdf.y = band_top;
    // The same `address_lines`/`email_line` decisions the page draws from, so
    // the two documents cannot show a client different lines.
    let mut client_lines = address_lines(client.billing_address.as_deref().unwrap_or_default());
    if let Some(email) = email_line(client.email.as_deref()) {
        client_lines.push(email);
    }
    pdf.party_block("Invoice For", &client.name, &client_lines, true);
    // The item table stands clear of the band above it, or its rows read as
    // part of the client block rather than as the invoice.
    pdf.y = pdf.y.max(left_bottom) + 14.0;

    let cols = &[
        Col {
            width: 97.8,
            align: Align::Left,
        },
        Col {
            width: 20.0,
            align: Align::Right,
        },
        Col {
            width: 30.0,
            align: Align::Right,
        },
        Col {
            width: 30.0,
            align: Align::Right,
        },
    ];
    let table_top = pdf.y - 3.5;
    let table_page = pdf.page_no;
    // The header is a row like any other and gets the same air. `table_header`
    // itself is shared with the nine report renderers, whose metrics are not
    // this task's to move.
    pdf.y += CELL_PAD_Y;
    pdf.table_header(cols, &["Description", "Quantity", "Unit Price", "Amount"]);

    for (index, item) in items.iter().enumerate() {
        let qty = item.quantity.to_string();
        let rate = money(item.unit_amount);
        let amount = money(item.line_total);
        pdf.item_row(
            cols,
            &[&item.description, &qty, &rate, &amount],
            row_is_shaded(index),
        );
    }
    // Column dividers, drawn once the table's extent is known. Skipped when the
    // rows paginated, since a rule spanning a page break would be drawn on the
    // wrong page — and `y` alone cannot say whether they did, because it resets
    // at the top of each new page and can pass the `> table_top` test again.
    if pdf.page_no == table_page && pdf.y > table_top {
        let mut divider = MARGIN_LEFT;
        for col in &cols[..cols.len() - 1] {
            divider += col.width;
            pdf.vline(divider, table_top, pdf.y - 3.5);
        }
    }
    pdf.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
    pdf.y += 6.0;

    // --- the money block, right-aligned under the Amount column -------------
    // Which lines exist is `MoneySummary::lines()`'s decision, taken once for
    // both documents. Only the total names the currency, which is where this
    // document has always put it.
    let figure_right = PAGE_W - MARGIN_RIGHT - COL_PAD;
    for line in summary.lines() {
        let label = if line.label == "Total" {
            format!("Total ({})", invoice.currency)
        } else {
            line.label.to_string()
        };
        // The rows the payment block introduced are new to both documents, so
        // they read the same on both — `USD 60.00`, the page's own form. The
        // older rows keep this document's `$` convention; reconciling those is
        // TASK-87's, and widening it here would restyle every invoice ever sent.
        let amount = if line.payment_row {
            format!("{} {:.2}", invoice.currency, line.amount)
        } else {
            money(line.amount)
        };
        // One size for every line; weight alone says which one matters. Two
        // lines set large with a small one between them read as two headlines
        // and a whisper rather than as a column of figures.
        pdf.ensure_space(ROW_H);
        pdf.text_right(&label, figure_right - 35.0, FONT_SIZE, line.emphasis);
        pdf.text_right(&amount, figure_right, FONT_SIZE, line.emphasis);
        pdf.y += ROW_H + 1.0;
    }

    // --- the foot ------------------------------------------------------------
    pdf.blank_row();
    pdf.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
    pdf.y += 6.0;

    // `NOTE_COLS` is the full printable width. These blocks are prose, and
    // prose set to the description column's half-measure runs to three short
    // lines where it should run to one.
    if let Some(notes) = &invoice.notes {
        pdf.section_label("Notes");
        pdf.table_row_wrapped(NOTE_COLS, &[notes], false, FONT_SIZE);
        pdf.blank_row();
    }
    // The block only when `due_value` did not already print the terms beside
    // the date — the page's rule, from the same function.
    if let Some(terms) = terms_block_text(invoice) {
        pdf.section_label("Terms");
        pdf.table_row_wrapped(NOTE_COLS, &[terms], false, FONT_SIZE);
        pdf.blank_row();
    }
    let instructions = payment_lines(payment_instructions);
    if !instructions.is_empty() {
        pdf.section_label("Payment");
        for line in instructions {
            pdf.table_row_wrapped(NOTE_COLS, &[line], false, FONT_SIZE);
        }
    }

    pdf.into_bytes()
}

/// A real PNG, painted by `image` so a decoder has something genuine to read.
/// Transparent and, at the sizes the tests ask for, wider than it is tall — the
/// shape printpdf's soft-mask path gets wrong.
#[cfg(all(test, feature = "pdf"))]
pub(crate) fn transparent_png(width: u32, height: u32) -> Vec<u8> {
    let mut buffer = image_crate::RgbaImage::new(width, height);
    for (x, _y, pixel) in buffer.enumerate_pixels_mut() {
        *pixel = image_crate::Rgba([200, 30, 30, if x % 2 == 0 { 0 } else { 255 }]);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    image_crate::DynamicImage::ImageRgba8(buffer)
        .write_to(&mut out, image_crate::ImageOutputFormat::Png)
        .unwrap();
    out.into_inner()
}

/// That PNG as the `company_logo` metadata value would hold it.
#[cfg(all(test, feature = "pdf"))]
pub(crate) fn logo_uri(width: u32, height: u32) -> String {
    use base64::Engine as _;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(transparent_png(width, height))
    )
}

/// Every image XObject a rendered document carries, as its dictionary. The seam
/// asserts through this rather than reaching into `lopdf` itself, which is only
/// in scope here.
#[cfg(test)]
pub(crate) fn image_xobjects(bytes: &[u8]) -> Vec<lopdf::Dictionary> {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered pdf parses");
    doc.objects
        .values()
        .filter_map(|object| object.as_stream().ok())
        .filter(|stream| {
            stream
                .dict
                .get(b"Subtype")
                .and_then(|s| s.as_name())
                .is_ok_and(|name| name == b"Image")
        })
        .map(|stream| stream.dict.clone())
        .collect()
}

/// The text a rendered document's content streams carry, in draw order. Tests
/// assert on what a PDF *says*, not merely that it parses.
#[cfg(test)]
pub(crate) fn extract_text(bytes: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered pdf parses");
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    doc.extract_text(&pages).expect("rendered pdf carries text")
}

/// One page's decompressed content stream, in page order.
#[cfg(test)]
fn page_streams(bytes: &[u8]) -> Vec<String> {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered pdf parses");
    doc.get_pages()
        .values()
        .map(|id| {
            String::from_utf8_lossy(&doc.get_page_content(*id).expect("page content")).into_owned()
        })
        .collect()
}

#[cfg(test)]
const PT_PER_MM: f32 = 72.0 / 25.4;

/// One string a rendered document drew: where, at what size, and what it said.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct Drawn {
    /// Millimetres from the left edge.
    pub x: f32,
    /// Millimetres from the bottom edge.
    pub y: f32,
    /// The point size it was drawn at, which is what its width is measured in.
    pub size: f32,
    pub text: String,
}

/// Every string a rendered document draws, per page.
///
/// `extract_text` answers what a document *says*; this answers *where* and *how
/// big*, which is the whole of what a column bound and a page break are about.
#[cfg(test)]
pub(crate) fn drawn_text(bytes: &[u8]) -> Vec<Vec<Drawn>> {
    page_streams(bytes)
        .iter()
        .map(|stream| {
            let mut out = Vec::new();
            let mut at = (0.0f32, 0.0f32);
            let mut size = 0.0f32;
            for line in stream.lines().map(str::trim) {
                if let Some(rest) = line.strip_suffix(" Tf") {
                    size = rest
                        .rsplit_once(' ')
                        .and_then(|(_, s)| s.parse::<f32>().ok())
                        .unwrap_or(size);
                } else if let Some(rest) = line.strip_suffix(" Td") {
                    let mut parts = rest.split_whitespace();
                    if let (Some(x), Some(y)) = (parts.next(), parts.next()) {
                        at = (
                            x.parse::<f32>().unwrap_or(0.0) / PT_PER_MM,
                            y.parse::<f32>().unwrap_or(0.0) / PT_PER_MM,
                        );
                    }
                } else if let Some(hex) = line
                    .strip_suffix("> Tj")
                    .and_then(|rest| rest.strip_prefix('<'))
                {
                    let text: String = hex
                        .as_bytes()
                        .chunks(2)
                        .filter_map(|pair| {
                            u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()
                        })
                        .map(|b| b as char)
                        .collect();
                    out.push(Drawn {
                        x: at.0,
                        y: at.1,
                        size,
                        text,
                    });
                }
            }
            out
        })
        .collect()
}

/// Every straight line a rendered document draws, per page, as
/// `(x1, y1, x2, y2)` in millimetres from the bottom-left.
#[cfg(test)]
pub(crate) fn drawn_lines(bytes: &[u8]) -> Vec<Vec<(f32, f32, f32, f32)>> {
    page_streams(bytes)
        .iter()
        .map(|stream| {
            let mut out = Vec::new();
            let mut start = None;
            for line in stream.lines().map(str::trim) {
                let point = |rest: &str| {
                    let mut parts = rest.split_whitespace();
                    let x = parts.next()?.parse::<f32>().ok()? / PT_PER_MM;
                    let y = parts.next()?.parse::<f32>().ok()? / PT_PER_MM;
                    Some((x, y))
                };
                if let Some(rest) = line.strip_suffix(" m") {
                    start = point(rest);
                } else if let Some(rest) = line.strip_suffix(" l") {
                    if let (Some(from), Some(to)) = (start, point(rest)) {
                        out.push((from.0, from.1, to.0, to.1));
                    }
                }
            }
            out
        })
        .collect()
}

/// Every filled rectangle a rendered document draws, per page, as
/// `(x1, y1, x2, y2)` in millimetres from the bottom-left.
///
/// The zebra striping is a fill, not a rule, so `drawn_lines` cannot see it.
#[cfg(test)]
pub(crate) fn filled_rects(bytes: &[u8]) -> Vec<Vec<(f32, f32, f32, f32)>> {
    page_streams(bytes)
        .iter()
        .map(|stream| {
            let mut out = Vec::new();
            for line in stream.lines().map(str::trim) {
                // `x y w h re` followed by the fill operator.
                let Some(rest) = line.strip_suffix(" re") else {
                    continue;
                };
                let nums: Vec<f32> = rest
                    .split_whitespace()
                    .filter_map(|n| n.parse::<f32>().ok())
                    .map(|n| n / PT_PER_MM)
                    .collect();
                if let [x, y, w, h] = nums[..] {
                    out.push((x, y, x + w, y + h));
                }
            }
            out
        })
        .collect()
}

/// Every string a rendered document draws in the bold face.
///
/// Weight is what carries emphasis now that the money block is all one size, so
/// a test that could only see position and size could not check it.
#[cfg(test)]
pub(crate) fn bold_strings(bytes: &[u8]) -> Vec<String> {
    page_streams(bytes)
        .iter()
        .flat_map(|stream| {
            let mut out = Vec::new();
            let mut bold = false;
            for line in stream.lines().map(str::trim) {
                if line.ends_with(" Tf") {
                    bold = line.contains("Bold");
                } else if let Some(hex) = line
                    .strip_suffix("> Tj")
                    .and_then(|rest| rest.strip_prefix('<'))
                {
                    if bold {
                        out.push(
                            hex.as_bytes()
                                .chunks(2)
                                .filter_map(|pair| {
                                    u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()
                                })
                                .map(|b| b as char)
                                .collect(),
                        );
                    }
                }
            }
            out
        })
        .collect()
}

/// Every stroke colour a rendered document sets, as unit RGB.
#[cfg(test)]
pub(crate) fn stroke_colors(bytes: &[u8]) -> Vec<(f32, f32, f32)> {
    page_streams(bytes)
        .iter()
        .flat_map(|stream| {
            stream
                .lines()
                .map(str::trim)
                .filter_map(|line| {
                    let rest = line.strip_suffix(" RG")?;
                    let nums: Vec<f32> = rest
                        .split_whitespace()
                        .filter_map(|n| n.parse::<f32>().ok())
                        .collect();
                    match nums[..] {
                        [r, g, b] => Some((r, g, b)),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(all(test, feature = "pdf"))]
mod invoice_pdf_tests {
    use super::*;
    use crate::invoicing::document::row_is_shaded;
    use crate::models::{Client, Invoice, InvoiceLineItem};

    fn invoice() -> Invoice {
        Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: None,
            status: "draft".into(),
            currency: "USD".into(),
            subtotal: 100.0,
            tax: 0.0,
            total: 100.0,
            notes: None,
            terms: None,
            token: "t".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: None,
            voided_at: None,
        }
    }

    fn client() -> Client {
        Client {
            id: 1,
            name: "Acme".into(),
            email: None,
            billing_address: None,
            notes: None,
            archived_at: None,
        }
    }

    fn items() -> Vec<InvoiceLineItem> {
        vec![InvoiceLineItem {
            id: None,
            invoice_id: Some(1),
            description: "Work".into(),
            quantity: 1.0,
            unit_amount: 100.0,
            line_total: 100.0,
            position: 0,
        }]
    }

    use crate::invoicing::document::{company_block, parse_logo};

    /// One line item, nothing paid — what every test that is about something
    /// else wants.
    fn pdf_of(invoice: &Invoice, client: &Client, company: &str) -> Vec<u8> {
        let money = MoneySummary::of(invoice, 0.0);
        let block = company_block(company, "", "");
        render_invoice_pdf(invoice, client, &block, None, &items(), &money, "").unwrap()
    }

    fn text_of(invoice: &Invoice, client: &Client, paid: f64) -> String {
        let money = MoneySummary::of(invoice, paid);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes =
            render_invoice_pdf(invoice, client, &block, None, &items(), &money, "").unwrap();
        extract_text(&bytes)
    }

    use crate::pdf::{logo_uri, transparent_png};

    fn pdf_with_logo(logo_uri: &str) -> Vec<u8> {
        let logo = parse_logo(logo_uri).ok().flatten();
        let money = MoneySummary::of(&invoice(), 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        render_invoice_pdf(
            &invoice(),
            &client(),
            &block,
            logo.as_ref(),
            &items(),
            &money,
            "",
        )
        .unwrap()
    }

    fn rich_client() -> Client {
        Client {
            id: 1,
            name: "Acme".into(),
            email: Some("ap@acme.test".into()),
            billing_address: Some("123 Main St\nSpringfield, IL 62704".into()),
            notes: None,
            archived_at: None,
        }
    }

    #[test]
    fn the_client_block_carries_the_address_and_the_email() {
        let text = text_of(&invoice(), &rich_client(), 0.0);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        // The letterhead heads the document and the metadata band follows it;
        // there is no title line between them.
        assert!(at("Bluepeak LLC") < at("Invoice ID"));
        assert!(at("Invoice ID") < at("Invoice For"));
        assert!(at("Invoice For") < at("123 Main St"));
        assert!(at("123 Main St") < at("Springfield, IL 62704"));
        assert!(at("Springfield, IL 62704") < at("ap@acme.test"));
        assert!(at("ap@acme.test") < at("Description"));
    }

    #[test]
    fn an_absent_address_or_email_draws_no_line() {
        // The sparse client: name only.
        let text = text_of(&invoice(), &client(), 0.0);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        assert!(at("Invoice For") < at("Description"));
        assert!(!text.contains("Main St"), "got: {text}");
        assert!(!text.contains("@"), "no email line at all: {text}");
    }

    #[test]
    fn the_from_block_carries_the_address_and_the_phone() {
        let money = MoneySummary::of(&invoice(), 0.0);
        let block = company_block(
            "Bluepeak LLC",
            "P.O. Box 1234\nSpringfield, CA 90001",
            "619.555.0123",
        );
        let bytes =
            render_invoice_pdf(&invoice(), &client(), &block, None, &items(), &money, "").unwrap();
        let text = extract_text(&bytes);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        assert!(at("From") < at("P.O. Box 1234"));
        assert!(at("P.O. Box 1234") < at("Springfield, CA 90001"));
        assert!(at("Springfield, CA 90001") < at("ph. 619.555.0123"));
    }

    /// A company with a phone and nothing else still has a From block. The page
    /// draws one, and an unlabelled telephone number floating where a letterhead
    /// should be is not what "both documents agree" means.
    #[test]
    fn a_phone_only_company_still_gets_a_labelled_from_block() {
        let money = MoneySummary::of(&invoice(), 0.0);
        let block = company_block("", "", "619.555.0123");
        let bytes =
            render_invoice_pdf(&invoice(), &client(), &block, None, &items(), &money, "").unwrap();
        let text = extract_text(&bytes);
        assert!(text.contains("From"), "the label: {text}");
        assert!(text.contains("ph. 619.555.0123"), "the phone: {text}");
    }

    /// The vertical rule is what makes the block a block. It has to run past the
    /// last line it is bracketing, and the phone is a line like any other.
    #[test]
    fn the_from_rule_brackets_every_line_including_the_phone() {
        let money = MoneySummary::of(&invoice(), 0.0);
        let block = company_block(
            "Bluepeak LLC",
            "P.O. Box 1234\nSpringfield, CA 90001",
            "619.555.0123",
        );
        let bytes =
            render_invoice_pdf(&invoice(), &client(), &block, None, &items(), &money, "").unwrap();

        let phone_y = drawn_text(&bytes)[0]
            .iter()
            .find(|drawn| drawn.text.starts_with("ph."))
            .expect("the phone is drawn")
            .y;
        // The From block's rule is the topmost vertical one.
        let rule = drawn_lines(&bytes)[0]
            .iter()
            .filter(|(x1, _, x2, _)| (x1 - x2).abs() < 0.01)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .copied()
            .expect("a vertical rule");
        let bottom = rule.1.min(rule.3);
        assert!(
            bottom < phone_y,
            "the rule stops at {bottom} mm, above the phone at {phone_y} mm"
        );
    }

    /// A 43-character address line is wider than the party column. Drawn
    /// unwrapped it runs off the right edge of the page, and the client's
    /// address is the one block a document may not silently lose.
    #[test]
    fn a_long_client_address_line_stays_inside_its_column() {
        let mut client = rich_client();
        client.billing_address = Some("1600 Pennsylvania Avenue Northwest, Suite 4100".into());
        let money = MoneySummary::of(&invoice(), 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes =
            render_invoice_pdf(&invoice(), &client, &block, None, &items(), &money, "").unwrap();

        for drawn in &drawn_text(&bytes)[0] {
            if drawn.x < PARTY_TEXT_X - 0.01 {
                continue;
            }
            let right = drawn.x + approx_text_width(&drawn.text, drawn.size);
            assert!(
                right <= PAGE_W - MARGIN_RIGHT + 0.01,
                "{:?} runs to {right} mm, past the {} mm margin",
                drawn.text,
                PAGE_W - MARGIN_RIGHT
            );
        }
        let text = extract_text(&bytes);
        assert!(text.contains("1600 Pennsylvania"), "still drawn: {text}");
        assert!(text.contains("Suite 4100"), "and all of it: {text}");
    }

    /// The wordmark is 22 pt and sits left of the From block. A long business
    /// name printed at that size would overprint it.
    #[test]
    fn a_long_company_name_does_not_overprint_the_from_block() {
        let money = MoneySummary::of(&invoice(), 0.0);
        let name = "Bluepeak Integrated Bookkeeping and Advisory Services LLC";
        let block = company_block(name, "P.O. Box 1234", "");
        let bytes =
            render_invoice_pdf(&invoice(), &client(), &block, None, &items(), &money, "").unwrap();

        let page = drawn_text(&bytes);
        let mark = page[0]
            .iter()
            .find(|drawn| (drawn.x - MARGIN_LEFT).abs() < 0.01)
            .expect("the wordmark is drawn at the left margin");
        assert!(
            mark.x + approx_text_width(&mark.text, mark.size) <= PARTY_LABEL_X,
            "{:?} at {} pt reaches the party column at {PARTY_LABEL_X} mm",
            mark.text,
            mark.size
        );
        assert!(
            mark.size <= WORDMARK_SIZE,
            "it was shrunk, not grown: {}",
            mark.size
        );
    }

    /// A due date carrying long terms is the value most likely to overrun: it is
    /// two fields on one line, and the party column starts right beside it.
    #[test]
    fn a_long_metadata_value_does_not_overprint_the_party_column() {
        let mut inv = invoice();
        inv.due_date = Some("2026-09-05".into());
        inv.terms = Some("Net 30 from receipt of the signed acceptance certificate".into());
        let money = MoneySummary::of(&inv, 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes =
            render_invoice_pdf(&inv, &client(), &block, None, &items(), &money, "").unwrap();

        for drawn in &drawn_text(&bytes)[0] {
            if (drawn.x - META_VALUE_X).abs() > 0.01 {
                continue;
            }
            assert!(
                drawn.x + approx_text_width(&drawn.text, drawn.size) <= PARTY_LABEL_X,
                "{:?} reaches the party column at {PARTY_LABEL_X} mm",
                drawn.text
            );
        }
    }

    /// The column dividers are drawn once the table's extent is known. When the
    /// rows paginate there is no single extent, and `y` alone cannot say so —
    /// it resets on a new page, so a guard reading it would draw the dividers
    /// down the second page over rows that are not there.
    #[test]
    fn a_paginated_item_table_draws_no_dividers_on_the_wrong_page() {
        let mut inv = invoice();
        inv.subtotal = 6000.0;
        inv.total = 6000.0;
        let long: Vec<InvoiceLineItem> = (0..60)
            .map(|i| InvoiceLineItem {
                id: None,
                invoice_id: Some(1),
                description: format!("Line item number {i}"),
                quantity: 1.0,
                unit_amount: 100.0,
                line_total: 100.0,
                position: i,
            })
            .collect();
        let money = MoneySummary::of(&inv, 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes = render_invoice_pdf(&inv, &client(), &block, None, &long, &money, "").unwrap();

        let pages = drawn_lines(&bytes);
        assert!(pages.len() > 1, "the premise: the table paginated");
        for (index, lines) in pages.iter().enumerate() {
            let verticals = lines.iter().filter(|(x1, _, x2, _)| (x1 - x2).abs() < 0.01);
            for rule in verticals {
                let top = rule.1.max(rule.3);
                let bottom = rule.1.min(rule.3);
                let covered = drawn_text(&bytes)[index]
                    .iter()
                    .any(|drawn| drawn.y >= bottom - 0.01 && drawn.y <= top + 0.01);
                assert!(
                    covered,
                    "page {index} carries a rule from {top} to {bottom} mm with no text beside it"
                );
            }
        }
    }

    /// The reference invoice has no title line: the letterhead is the masthead
    /// and the metadata band carries the identifier. Printing "Invoice #1248"
    /// above a row that reads `Invoice ID  1248` says the same thing twice.
    #[test]
    fn the_number_is_printed_once_in_the_metadata_band() {
        let bytes = pdf_of(&invoice(), &client(), "Bluepeak LLC");
        let text = extract_text(&bytes);
        assert!(!text.contains("Invoice #"), "no visible title line: {text}");
        assert_eq!(
            text.matches("1248").count(),
            1,
            "the number appears once, in the metadata band: {text}"
        );
        // File metadata is not visible layout, and a viewer's window title and
        // a browser's suggested filename both read it.
        assert_eq!(document_title_of(&bytes), "Bluepeak LLC - Invoice #1248");
    }

    /// The zebra is what lets a reader track one row across four columns, so it
    /// has to be a fill behind the row rather than a rule between rows — and it
    /// has to keep alternating after a page break, where the row index carries
    /// on but the page's geometry starts over.
    #[test]
    fn every_other_item_row_is_shaded_on_every_page() {
        let mut inv = invoice();
        inv.subtotal = 6000.0;
        inv.total = 6000.0;
        let long: Vec<InvoiceLineItem> = (0..60)
            .map(|i| InvoiceLineItem {
                id: None,
                invoice_id: Some(1),
                description: format!("Line item number {i}"),
                quantity: 1.0,
                unit_amount: 100.0,
                line_total: 100.0,
                position: i,
            })
            .collect();
        let money = MoneySummary::of(&inv, 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes = render_invoice_pdf(&inv, &client(), &block, None, &long, &money, "").unwrap();

        let pages = filled_rects(&bytes);
        assert!(pages.len() > 1, "the premise: the table paginated");
        let shaded: usize = pages.iter().map(Vec::len).sum();
        assert_eq!(
            shaded,
            (0..long.len()).filter(|i| row_is_shaded(*i)).count(),
            "one fill per shaded row, no more and no fewer"
        );
        for (index, page) in pages.iter().enumerate() {
            assert!(
                !page.is_empty(),
                "page {index} carries rows but no striping"
            );
            for rect in page {
                let covered = drawn_text(&bytes)[index]
                    .iter()
                    .any(|drawn| drawn.y >= rect.1 - 0.01 && drawn.y <= rect.3 + 0.01);
                assert!(covered, "page {index} shades a band with no row in it");
            }
        }
    }

    /// Every rule on this document is one grey, and it is the grey the page
    /// uses. Two renderers each naming their own is how one document's table
    /// ends up caged in near-black while the other's is a whisper.
    #[test]
    fn every_rule_is_drawn_in_the_shared_border_grey() {
        let bytes = pdf_of(&invoice(), &rich_client(), "Bluepeak LLC");
        let (r, g, b) = crate::invoicing::document::BORDER_GRAY.unit_rgb();
        let strokes = stroke_colors(&bytes);
        assert!(!strokes.is_empty(), "the document draws rules at all");
        for stroke in &strokes {
            assert!(
                (stroke.0 - r).abs() < 0.01
                    && (stroke.1 - g).abs() < 0.01
                    && (stroke.2 - b).abs() < 0.01,
                "a rule is drawn in {stroke:?}, not the shared grey"
            );
        }
    }

    /// The band above the table needs room, or the item rows read as part of
    /// the client block rather than as the invoice.
    #[test]
    fn the_item_table_stands_clear_of_the_band_above_it() {
        let bytes = pdf_of(&invoice(), &rich_client(), "Bluepeak LLC");
        let page = &drawn_text(&bytes)[0];
        let lowest_band_line = page
            .iter()
            .filter(|d| d.text == "ap@acme.test")
            .map(|d| d.y)
            .fold(f32::INFINITY, f32::min);
        let header = page
            .iter()
            .find(|d| d.text == "Description")
            .expect("the table header")
            .y;
        assert!(
            lowest_band_line - header >= 14.0,
            "only {} mm between the band and the table",
            lowest_band_line - header
        );
    }

    /// One size for every money line, and weight — not size — carries the
    /// emphasis. The block reads as a short column of figures with the one that
    /// matters picked out, rather than two headlines with a whisper between.
    #[test]
    fn the_money_block_is_one_size_with_only_its_bottom_line_bold() {
        let mut inv = invoice();
        inv.subtotal = 100.0;
        inv.total = 100.0;
        let money = MoneySummary::of(&inv, 40.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes =
            render_invoice_pdf(&inv, &client(), &block, None, &items(), &money, "").unwrap();

        let labels = ["Total", "Paid", "Balance due"];
        let page = drawn_text(&bytes);
        let drawn: Vec<&Drawn> = page[0]
            .iter()
            .filter(|d| labels.contains(&d.text.split(" (").next().unwrap_or("")))
            .collect();
        assert_eq!(drawn.len(), 3, "three money labels: {drawn:?}");
        for line in &drawn {
            assert_eq!(
                line.size, FONT_SIZE,
                "{:?} is set at {}, not the body size",
                line.text, line.size
            );
        }
        let bold = bold_strings(&bytes);
        assert!(
            bold.iter().any(|t| t == "Balance due"),
            "the balance is the line that shouts: {bold:?}"
        );
        for label in ["Total (USD)", "Paid"] {
            assert!(
                !bold.iter().any(|t| t == label),
                "{label} is still bold: {bold:?}"
            );
        }
    }

    /// The foot blocks are prose, and prose set to half the measure runs to
    /// three short lines where it should run to one or two.
    #[test]
    fn the_foot_blocks_use_the_full_printable_width() {
        let mut inv = invoice();
        inv.notes = Some(
            "Thank you for your business, and please do get in touch with any questions at all."
                .into(),
        );
        let money = MoneySummary::of(&inv, 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes = render_invoice_pdf(
            &inv,
            &client(),
            &block,
            None,
            &items(),
            &money,
            "Bank transfer to Example Bank, quoting the invoice number, or a cheque by post.",
        )
        .unwrap();

        // Nothing under the foot rule wraps: at the full measure both fit one
        // line each, where the old half-width column broke them.
        let text = extract_text(&bytes);
        for whole in [
            "Thank you for your business, and please do get in touch with any questions at all.",
            "Bank transfer to Example Bank, quoting the invoice number, or a cheque by post.",
        ] {
            assert!(text.contains(whole), "wrapped: {text}");
        }
    }

    /// Rows read cramped when the type sits hard against its rules. The band
    /// has to grow with the padding, or the striping stops covering the row it
    /// belongs to.
    #[test]
    fn item_rows_are_padded_and_their_bands_cover_the_whole_row() {
        let money = MoneySummary::of(&invoice(), 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let two = vec![
            items()[0].clone(),
            InvoiceLineItem {
                description: "Second".into(),
                ..items()[0].clone()
            },
        ];
        let bytes =
            render_invoice_pdf(&invoice(), &client(), &block, None, &two, &money, "").unwrap();

        let band = filled_rects(&bytes)[0]
            .first()
            .copied()
            .expect("the second row is shaded");
        let height = band.3 - band.1;
        assert!(
            height > ROW_H + 1.0,
            "the band is {height} mm for a one-line row — no padding in it"
        );

        // The shaded row's own text sits inside its band, clear of both edges.
        let baseline = drawn_text(&bytes)[0]
            .iter()
            .find(|d| d.text == "Second")
            .expect("the second row is drawn")
            .y;
        assert!(
            baseline > band.1 + 0.5 && baseline < band.3 - 0.5,
            "the row's type is not inside its band: {baseline} in {band:?}"
        );
    }

    #[test]
    fn an_unset_company_draws_no_from_block() {
        let text = extract_text(&pdf_of(&invoice(), &client(), ""));
        assert!(!text.contains("From"), "got: {text}");
        assert!(!text.contains("ph."), "got: {text}");
        // The bands after it still start where they should.
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        assert!(at("Invoice ID") < at("Invoice For"));
        assert!(at("Invoice For") < at("Description"));
    }

    #[test]
    fn the_metadata_column_is_the_shared_one() {
        let mut inv = invoice();
        inv.due_date = Some("2026-09-03".into());
        inv.terms = Some("Net 30".into());
        let text = text_of(&inv, &client(), 0.0);

        let mut at = 0;
        for row in crate::invoicing::document::meta_rows(&inv) {
            for needle in [row.label, row.value.as_str()] {
                let found = text[at..]
                    .find(needle)
                    .unwrap_or_else(|| panic!("{needle} missing or out of order: {text}"));
                at += found + needle.len();
            }
        }
        assert!(text.contains("2026-09-03 (Net 30)"), "got: {text}");
    }

    #[test]
    fn the_item_table_says_quantity_and_unit_price() {
        let text = text_of(&invoice(), &client(), 0.0);
        assert!(text.contains("Quantity"), "got: {text}");
        assert!(text.contains("Unit Price"), "got: {text}");
        assert!(!text.contains("Rate"), "got: {text}");
    }

    #[test]
    fn a_configured_logo_is_embedded_as_an_image() {
        let bytes = pdf_with_logo(&logo_uri(400, 60));
        assert_eq!(image_xobjects(&bytes).len(), 1, "one image, the logo");
    }

    /// The whole reason the flattening exists. printpdf 0.7 sizes a soft mask
    /// from the image's *width*, so a wide transparent logo embeds a mask
    /// declaring `width x width` samples. Handing it `Rgb8` makes that path
    /// unreachable, and `/SMask null` is the proof.
    #[test]
    fn nothing_handed_to_printpdf_is_ever_rgba() {
        let prepared = prepare_logo(&transparent_png(400, 60)).expect("a decodable png");
        assert!(
            matches!(prepared, image_crate::DynamicImage::ImageRgb8(_)),
            "printpdf must never see an alpha channel"
        );

        let dictionaries = image_xobjects(&pdf_with_logo(&logo_uri(400, 60)));
        let dict = dictionaries.first().expect("the logo");
        assert!(
            matches!(dict.get(b"SMask").unwrap(), lopdf::Object::Null),
            "an alpha channel reached printpdf"
        );
        assert_eq!(dict.get(b"Width").unwrap().as_i64().unwrap(), 400);
        assert_eq!(dict.get(b"Height").unwrap().as_i64().unwrap(), 60);
    }

    #[test]
    fn a_transparent_logo_is_flattened_onto_white() {
        use image_crate::GenericImageView as _;
        let prepared = prepare_logo(&transparent_png(4, 1)).expect("a decodable png");
        // Even columns are fully transparent and become the page they sit on;
        // odd ones keep their colour.
        assert_eq!(prepared.get_pixel(0, 0).0[..3], [255, 255, 255]);
        assert_eq!(prepared.get_pixel(1, 0).0[..3], [200, 30, 30]);
    }

    #[test]
    fn the_logo_is_bounded_and_keeps_its_aspect_ratio() {
        // A 10:1 wordmark binds on width; a 1:10 tower binds on height. Both
        // fit the box, and neither is distorted.
        for (px_w, px_h) in [(1200u32, 120u32), (120, 1200)] {
            let bytes = pdf_with_logo(&logo_uri(px_w, px_h));
            let doc = lopdf::Document::load_mem(&bytes).unwrap();
            let (scale_w, scale_h) = drawn_logo_size(&doc);
            assert!(
                scale_w <= LOGO_MAX_W + 0.01 && scale_h <= LOGO_MAX_H + 0.01,
                "{px_w}x{px_h} drew {scale_w}x{scale_h}mm, outside the box"
            );
            let wanted = px_w as f32 / px_h as f32;
            assert!(
                ((scale_w / scale_h) - wanted).abs() < 0.01,
                "{px_w}x{px_h} drew at aspect {}, wanted {wanted}",
                scale_w / scale_h
            );
            // One of the two dimensions fills the box, or the bound is not one.
            assert!(
                (scale_w - LOGO_MAX_W).abs() < 0.01 || (scale_h - LOGO_MAX_H).abs() < 0.01,
                "{px_w}x{px_h} drew {scale_w}x{scale_h}mm, filling neither dimension"
            );
        }
    }

    /// The width and height in mm of the one image the document draws, read
    /// back out of the content stream's scaling matrix.
    fn drawn_logo_size(doc: &lopdf::Document) -> (f32, f32) {
        let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
        let content = doc
            .get_page_content(*doc.get_pages().get(&pages[0]).unwrap())
            .unwrap();
        let operations = lopdf::content::Content::decode(&content)
            .unwrap()
            .operations;
        let matrix = operations
            .iter()
            .find(|op| op.operator == "cm")
            .expect("an image transform");
        let number = |i: usize| matrix.operands[i].as_float().unwrap();
        // The first `cm` is `Scale(w_pt, h_pt)`; 1 pt is 25.4/72 mm.
        (number(0) * 25.4 / 72.0, number(3) * 25.4 / 72.0)
    }

    /// A logo may never cost an invoice. Every way one can be unusable ends
    /// with a rendered document headed by the company name.
    #[test]
    fn an_unusable_logo_falls_back_to_the_wordmark_rather_than_failing() {
        use base64::Engine as _;
        let over_cap = base64::engine::general_purpose::STANDARD.encode(vec![0u8; 200 * 1024]);
        let undecodable = {
            // Right magic bytes and a readable IHDR, but no image data behind
            // it: `parse_logo` accepts it and the decoder cannot.
            let mut bytes = vec![
                0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H',
                b'D', b'R', 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
                0x00,
            ];
            bytes.extend_from_slice(&[0u8; 4]);
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            )
        };
        for bad in [
            "data:image/png;base64,bm90IGEgcG5n".to_string(),
            format!("data:image/png;base64,{over_cap}"),
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=".to_string(),
            "not a data uri".to_string(),
            undecodable,
        ] {
            let bytes = pdf_with_logo(&bad);
            assert!(bytes.starts_with(b"%PDF"), "{bad} failed the render");
            assert!(
                image_xobjects(&bytes).is_empty(),
                "{bad} embedded something"
            );
            assert!(
                extract_text(&bytes).contains("Bluepeak LLC"),
                "{bad} left no wordmark"
            );
        }
    }

    #[test]
    fn the_payment_instructions_are_printed_under_the_foot_rule() {
        let money = MoneySummary::of(&invoice(), 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes = render_invoice_pdf(
            &invoice(),
            &client(),
            &block,
            None,
            &items(),
            &money,
            "Wells Fargo\nRouting 121000248",
        )
        .unwrap();
        let text = extract_text(&bytes);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        assert!(at("Total") < at("Payment"));
        assert!(at("Payment") < at("Wells Fargo"));
        assert!(at("Wells Fargo") < at("Routing 121000248"));
    }

    #[test]
    fn no_payment_instructions_draw_no_payment_heading() {
        let text = text_of(&invoice(), &client(), 0.0);
        assert!(!text.contains("Payment"), "got: {text}");
    }

    #[test]
    fn the_money_block_is_the_shared_one() {
        let mut inv = invoice();
        inv.subtotal = 100.0;
        inv.tax = 8.25;
        inv.total = 108.25;
        let money = MoneySummary::of(&inv, 0.0);
        let text = text_of(&inv, &client(), 0.0);

        let mut at = 0;
        for line in money.lines() {
            let label = if line.label == "Total" {
                "Total (USD)".to_string()
            } else {
                line.label.to_string()
            };
            let found = text[at..]
                .find(&label)
                .unwrap_or_else(|| panic!("{label} missing or out of order: {text}"));
            at += found + label.len();
        }
        assert_eq!(
            money.lines().len(),
            3,
            "tax brings the subtotal with it, and nothing was paid"
        );
    }

    #[test]
    fn a_paid_invoice_shows_paid_and_the_balance() {
        let text = text_of(&invoice(), &client(), 40.0);
        assert!(text.contains("Paid"), "got: {text}");
        assert!(text.contains("Balance due"), "got: {text}");
        assert!(text.contains("USD 60.00"), "got: {text}");
    }

    /// The rows the payment block introduced are new to both documents, so
    /// they read identically on both — `USD 60.00` — rather than this one
    /// keeping a `$` that cannot say which currency it means.
    #[test]
    fn the_payment_rows_name_the_currency_the_way_the_page_does() {
        let text = text_of(&invoice(), &client(), 40.0);
        assert!(text.contains("USD 40.00"), "paid: {text}");
        assert!(text.contains("USD 60.00"), "balance: {text}");
        assert!(
            !text.contains("$40.00"),
            "no dollar-only payment row: {text}"
        );
        assert!(
            !text.contains("$60.00"),
            "no dollar-only payment row: {text}"
        );
    }

    /// A non-USD invoice is the case a bare `$` gets wrong.
    #[test]
    fn a_non_usd_payment_row_says_which_currency_it_means() {
        let mut inv = invoice();
        inv.currency = "EUR".into();
        let text = text_of(&inv, &client(), 40.0);
        assert!(text.contains("EUR 60.00"), "got: {text}");
    }

    #[test]
    fn an_overpayment_draws_a_credit_row_and_no_negative_balance() {
        let text = text_of(&invoice(), &client(), 130.0);
        assert!(text.contains("Credit"), "got: {text}");
        assert!(text.contains("USD 30.00"), "got: {text}");
        assert!(text.contains("USD 0.00"), "the balance is zero: {text}");
        // The dates carry hyphens; what may never appear is a negative amount.
        assert!(!text.contains("USD -"), "no negative figure: {text}");
        assert!(!text.contains("$-"), "no negative figure: {text}");
    }

    /// A pasted-in address may not run off the bottom margin.
    #[test]
    fn a_very_long_address_is_clamped_to_what_the_page_can_hold() {
        let mut c = client();
        c.billing_address = Some(
            (1..=12)
                .map(|n| format!("Address line {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let text = text_of(&invoice(), &c, 0.0);

        assert!(text.contains("Address line 1"), "got: {text}");
        assert!(!text.contains("Address line 12"), "clamped: {text}");
        assert!(
            text.contains(crate::invoicing::document::ADDRESS_TRUNCATED),
            "the cut is shown: {text}"
        );
        // The block still ends where it should, with the dates after it.
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        assert!(at("Address line 1") < at("Description"));
    }

    /// The one thing this document deliberately does not carry. An emailed
    /// attachment cannot be recalled or republished, so a live charge link in it
    /// would outlive the settlement it was created for.
    #[test]
    fn no_live_payment_link_reaches_the_pdf() {
        let mut inv = invoice();
        inv.stripe_payment_link_url = Some("https://pay.stripe.test/x".into());
        let text = text_of(&inv, &client(), 0.0);
        assert!(!text.contains("pay.stripe.test"), "got: {text}");
        assert!(!text.contains("Pay online"), "got: {text}");
    }

    /// Not only no Stripe link: no address at all. A tokenized page URL printed
    /// as unclickable text is noise beside the figure that matters, and the
    /// email carries the live link.
    #[test]
    fn the_pdf_prints_no_url_at_all() {
        let mut inv = invoice();
        inv.stripe_payment_link_url = Some("https://pay.stripe.test/x".into());
        inv.published_at = Some("2026-08-04".into());
        inv.token = "abc123".into();
        let text = text_of(&inv, &client(), 0.0);
        for absence in ["http", "index.html", "abc123", "://"] {
            assert!(!text.contains(absence), "{absence} survived in: {text}");
        }
    }

    #[test]
    fn the_wordmark_heads_the_document_when_there_is_no_logo() {
        let bytes = pdf_of(&invoice(), &client(), "Bluepeak LLC");
        let text = extract_text(&bytes);

        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing: {text}"))
        };
        assert!(at("Bluepeak LLC") < at("Invoice ID"));
        assert!(at("Invoice ID") < at("Invoice For"));
        assert!(image_xobjects(&bytes).is_empty(), "no image was configured");
    }

    /// The pure header reader in `document.rs` has to agree with the decoder
    /// that actually embeds the image, or the two would drift.
    #[test]
    fn the_header_dimensions_match_what_the_decoder_reads() {
        use image_crate::GenericImageView as _;
        let png = transparent_png(37, 11);
        let logo = parse_logo(&logo_uri(37, 11)).unwrap().expect("a logo");
        let decoded = image_crate::load_from_memory(&png).unwrap();
        assert_eq!((logo.width, logo.height), decoded.dimensions());
        assert_eq!((logo.width, logo.height), (37, 11));
    }

    fn document_title_of(bytes: &[u8]) -> String {
        let doc = lopdf::Document::load_mem(bytes).unwrap();
        doc.trailer
            .get(b"Info")
            .and_then(|info| doc.get_dictionary(info.as_reference().unwrap()))
            .and_then(|info| info.get(b"Title"))
            .map(|t| String::from_utf8_lossy(t.as_str().unwrap()).into_owned())
            .expect("document info carries a title")
    }

    #[test]
    fn an_unset_company_leaves_a_text_only_header() {
        let bytes = pdf_of(&invoice(), &client(), "");
        let text = extract_text(&bytes);

        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing: {text}"))
        };
        assert!(at("Invoice ID") < at("Invoice For"));
        // Nothing visible names the company, and the Info title falls back to
        // the invoice alone.
        assert_eq!(document_title_of(&bytes), "Invoice #1248");
    }

    #[test]
    fn the_document_title_carries_the_company() {
        let bytes = pdf_of(&invoice(), &client(), "Bluepeak LLC");
        assert_eq!(document_title_of(&bytes), "Bluepeak LLC - Invoice #1248");
    }

    #[test]
    fn produces_nonempty_pdf() {
        let bytes = pdf_of(&invoice(), &client(), "");
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..4], b"%PDF");
    }

    #[test]
    fn renders_optional_fields() {
        let inv = Invoice {
            id: 2,
            number: 1249,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: Some("2026-09-03".into()),
            status: "sent".into(),
            currency: "USD".into(),
            subtotal: 100.0,
            tax: 8.25,
            total: 108.25,
            notes: Some("Thanks for your business.".into()),
            terms: Some("Net 30".into()),
            token: "t".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: None,
            voided_at: None,
        };
        let items = vec![InvoiceLineItem {
            id: None,
            invoice_id: Some(2),
            description: "Consulting engagement covering discovery, build, and handoff".into(),
            quantity: 2.0,
            unit_amount: 50.0,
            line_total: 100.0,
            position: 0,
        }];
        let money = MoneySummary::of(&inv, 0.0);
        let block = company_block("Bluepeak LLC", "", "");
        let bytes = render_invoice_pdf(&inv, &client(), &block, None, &items, &money, "").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_connection, init_db};

    fn test_db() -> (tempfile::TempDir, rusqlite::Connection) {
        let dir = tempfile::tempdir().unwrap();
        let conn = get_connection(&dir.path().join("test.db")).unwrap();
        init_db(&conn).unwrap();
        (dir, conn)
    }

    fn seed(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO accounts (name, account_type) VALUES ('Test', 'checking')",
            [],
        )
        .unwrap();
        let acct = conn.last_insert_rowid();
        let income_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Client Services'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let expense_cat: i64 = conn
            .query_row(
                "SELECT id FROM categories WHERE name = 'Software & Subscriptions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-15', 'Client payment', 1000.0, ?2)",
            rusqlite::params![acct, income_cat],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions (account_id, date, description, amount, category_id) \
             VALUES (?1, '2025-01-20', 'Adobe CC', -50.0, ?2)",
            rusqlite::params![acct, expense_cat],
        )
        .unwrap();
    }

    #[test]
    fn test_render_pnl_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_pnl(&conn, Some(2025), None, None, None).unwrap();
        let bytes = render_pnl(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_expenses_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_expense_breakdown(&conn, Some(2025), None).unwrap();
        let bytes = render_expenses(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_tax_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_tax_summary(&conn, Some(2025)).unwrap();
        let bytes = render_tax(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_cashflow_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_cashflow(&conn, Some(2025), None).unwrap();
        let bytes = render_cashflow(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_flagged_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let rows = get_flagged(&conn).unwrap();
        let bytes = render_flagged(&rows, "Test Corp").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_balance_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_balance(&conn).unwrap();
        let bytes = render_balance(&report, "Test Corp").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_aging_produces_pdf() {
        use crate::invoicing::invoices::{AgingBucket, AgingInvoice, AgingReport};
        let report = AgingReport {
            as_of: "2026-08-04".into(),
            buckets: vec![
                AgingBucket {
                    label: "current",
                    count: 0,
                    total: 0.0,
                },
                AgingBucket {
                    label: "31-60",
                    count: 1,
                    total: 100.0,
                },
            ],
            invoices: vec![AgingInvoice {
                number: 1248,
                client: "Acme Co".into(),
                due_date: "2026-06-20".into(),
                days_past_due: 45,
                bucket: "31-60",
                total: 100.0,
                paid: 0.0,
                balance: 100.0,
            }],
            outstanding: 100.0,
        };
        let bytes = render_aging(&report, "Test Corp").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_render_k1_produces_pdf() {
        let (_dir, conn) = test_db();
        seed(&conn);
        let report = get_k1_prep(&conn, Some(2025)).unwrap();
        let bytes = render_k1(&report, "Test Corp", "FY 2025").unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}
