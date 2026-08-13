use std::io::BufWriter;

use printpdf::*;

use crate::error::{NigelError, Result};
use crate::fmt::money;
use crate::invoicing::document::{address_lines, email_line, MoneySummary};
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
const COL_PAD: f32 = 4.0;
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

    fn hline(&self, x1: f32, x2: f32) {
        let layer = self
            .doc
            .get_page(self.current_page)
            .get_layer(self.current_layer);
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

/// `company` is the operator's own name — the `company_name` metadata key the
/// HTML page resolves through `Branding`. Empty means unset, and the document
/// is headed by the invoice number alone.
/// The invoice as the client's email attachment carries it.
///
/// It deliberately carries **no payment link**. An emailed attachment cannot be
/// recalled or republished, so a live charge link in it would survive the
/// settlement it was created for — the same reasoning that makes void deactivate
/// links. Paying online is the published page's job, and the page is the one
/// artifact a republish can correct.
pub fn render_invoice_pdf(
    invoice: &Invoice,
    client: &Client,
    items: &[InvoiceLineItem],
    company: &str,
    summary: &MoneySummary,
) -> Result<Vec<u8>> {
    let title = format!("Invoice #{}", invoice.number);
    let mut pdf = PdfWriter::new(&document_title(&title, company))?;

    pdf.text(&title, MARGIN_LEFT, TITLE_SIZE, true);
    pdf.y += 7.0;
    if !company.is_empty() {
        pdf.text(company, MARGIN_LEFT, SUBTITLE_SIZE, true);
        pdf.y += 5.0;
    }
    pdf.text(
        &format!("Billed to: {}", client.name),
        MARGIN_LEFT,
        SUBTITLE_SIZE,
        false,
    );
    pdf.y += 5.0;
    // One row per typed line, matching how the page renders the same address.
    // An absent one draws nothing, so `Issued:` follows the name directly.
    for line in address_lines(client.billing_address.as_deref().unwrap_or_default()) {
        pdf.text(line, MARGIN_LEFT, SUBTITLE_SIZE, false);
        pdf.y += 5.0;
    }
    if let Some(email) = email_line(client.email.as_deref()) {
        pdf.text(email, MARGIN_LEFT, SUBTITLE_SIZE, false);
        pdf.y += 5.0;
    }
    pdf.text(
        &format!("Issued: {}", invoice.issue_date),
        MARGIN_LEFT,
        SUBTITLE_SIZE,
        false,
    );
    pdf.y += 5.0;
    if let Some(due) = &invoice.due_date {
        pdf.text(&format!("Due: {due}"), MARGIN_LEFT, SUBTITLE_SIZE, false);
        pdf.y += 5.0;
    }
    pdf.hline(MARGIN_LEFT, PAGE_W - MARGIN_RIGHT);
    pdf.y += 5.0;

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
    pdf.table_header(cols, &["Description", "Qty", "Rate", "Amount"]);

    for item in items {
        let qty = item.quantity.to_string();
        let rate = money(item.unit_amount);
        let amount = money(item.line_total);
        pdf.table_row_wrapped(
            cols,
            &[&item.description, &qty, &rate, &amount],
            false,
            FONT_SIZE,
        );
    }

    pdf.separator();
    // Which money lines exist is `MoneySummary::lines()`'s decision, taken once
    // for both documents. Only the total names the currency, which is where
    // this document has always put it.
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
        pdf.table_row(cols, &[&label, "", "", &amount], line.emphasis);
    }

    if let Some(notes) = &invoice.notes {
        pdf.blank_row();
        pdf.section_label("Notes");
        pdf.table_row_wrapped(&cols[..1], &[notes], false, FONT_SIZE);
    }
    if let Some(terms) = &invoice.terms {
        pdf.blank_row();
        pdf.section_label("Terms");
        pdf.table_row_wrapped(&cols[..1], &[terms], false, FONT_SIZE);
    }

    pdf.into_bytes()
}

/// The text a rendered document's content streams carry, in draw order. Tests
/// assert on what a PDF *says*, not merely that it parses.
#[cfg(test)]
pub(crate) fn extract_text(bytes: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(bytes).expect("rendered pdf parses");
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    doc.extract_text(&pages).expect("rendered pdf carries text")
}

#[cfg(all(test, feature = "pdf"))]
mod invoice_pdf_tests {
    use super::*;
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

    /// One line item, nothing paid — what every test that is about something
    /// else wants.
    fn pdf_of(invoice: &Invoice, client: &Client, company: &str) -> Vec<u8> {
        let money = MoneySummary::of(invoice, 0.0);
        render_invoice_pdf(invoice, client, &items(), company, &money).unwrap()
    }

    fn text_of(invoice: &Invoice, client: &Client, paid: f64) -> String {
        let money = MoneySummary::of(invoice, paid);
        let bytes = render_invoice_pdf(invoice, client, &items(), "Bluepeak LLC", &money).unwrap();
        extract_text(&bytes)
    }

    fn rich_client() -> Client {
        Client {
            id: 1,
            name: "Acme".into(),
            email: Some("ap@acme.test".into()),
            billing_address: Some("123 Main St\nSpringfield, IL 62704".into()),
            notes: None,
        }
    }

    #[test]
    fn the_client_block_carries_the_address_and_the_email() {
        let text = text_of(&invoice(), &rich_client(), 0.0);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        assert!(at("Invoice #1248") < at("Bluepeak LLC"));
        assert!(at("Bluepeak LLC") < at("Billed to: Acme"));
        assert!(at("Billed to: Acme") < at("123 Main St"));
        assert!(at("123 Main St") < at("Springfield, IL 62704"));
        assert!(at("Springfield, IL 62704") < at("ap@acme.test"));
        assert!(at("ap@acme.test") < at("Issued:"));
    }

    #[test]
    fn an_absent_address_or_email_draws_no_line() {
        // The sparse client: name only.
        let text = text_of(&invoice(), &client(), 0.0);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing {needle}: {text}"))
        };
        assert!(at("Billed to: Acme") < at("Issued:"));
        assert!(!text.contains("Main St"), "got: {text}");
        assert!(!text.contains("@"), "no email line at all: {text}");
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
        assert!(at("Address line 1") < at("Issued:"));
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

    #[test]
    fn the_company_name_heads_the_document() {
        let bytes = pdf_of(&invoice(), &client(), "Bluepeak LLC");
        let text = extract_text(&bytes);

        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("missing: {text}"))
        };
        assert!(at("Invoice #1248") < at("Bluepeak LLC"));
        assert!(at("Bluepeak LLC") < at("Billed to: Acme"));
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
        assert!(at("Invoice #1248") < at("Billed to: Acme"));
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
        let bytes = render_invoice_pdf(&inv, &client(), &items, "Bluepeak LLC", &money).unwrap();
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
