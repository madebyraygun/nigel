//! What both client-facing documents say about money and about the client,
//! decided once.
//!
//! The page and the PDF are rendered by two different renderers from the same
//! row, and the only way they can be trusted to agree is for the decision — the
//! figures, and which of them appear at all — to live above both of them.

use base64::Engine as _;

use crate::error::{NigelError, Result};
use crate::invoicing::invoices::{is_settled, CENT_SLACK};
use crate::models::Invoice;

/// How many lines of a client's billing address a document will draw.
///
/// The PDF draws one row per line at a fixed offset with no page-break logic,
/// so an address pasted out of a spreadsheet would run off the bottom margin.
/// Six is a generous postal address anywhere; past that the block is telling
/// the reader something other than where to send a cheque.
pub const MAX_ADDRESS_LINES: usize = 6;

/// What stands in for the lines that did not fit.
///
/// Three ASCII dots rather than an ellipsis character: the PDF draws with the
/// built-in Helvetica, whose WinAnsi encoding does not carry `U+2026` through
/// to a reader intact, and the whole point of clamping in one place is that
/// both documents show the same thing.
pub const ADDRESS_TRUNCATED: &str = "...";

/// A colour both documents draw the same thing in.
///
/// One value, two renderings: the page needs a hex string for CSS, the PDF needs
/// three floats between 0 and 1. Neither renderer is allowed to name a colour of
/// its own, which is what stops one document's rules going grey while the
/// other's stay black.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl DocumentColor {
    const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// The default a PDF draws in when nothing has said otherwise.
    pub const BLACK: Self = Self::new(0, 0, 0);

    /// The CSS form, for a template author and for the stock page's stylesheet.
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// The PDF form: each channel as a fraction of full intensity.
    pub fn unit_rgb(&self) -> (f32, f32, f32) {
        (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        )
    }
}

/// The one grey every structural rule on both documents is drawn in — the party
/// blocks' vertical rules, the item table's grid, the foot rule.
///
/// A medium neutral, deliberately well clear of the body text: a rule is
/// structure, not type, and drawing the two in the same near-black makes a table
/// read as a cage. It is also well clear of white, so it survives a fax-grade
/// printer, which the near-invisible hairlines it replaces did not.
pub const BORDER_GRAY: DocumentColor = DocumentColor::new(0x90, 0x90, 0x90);

/// The tint behind every other line-item row.
///
/// Light enough that the body text over it keeps its full contrast, and dark
/// enough to survive a printer that dithers: the striping is there to let a
/// reader track a long row across four columns, so it has to be visible on paper
/// and must never make a figure harder to read.
pub const ROW_SHADE: DocumentColor = DocumentColor::new(0xf4, 0xf4, 0xf4);

/// Whether the line-item row at `index` carries the zebra tint.
///
/// The second row and every other one after it, so the first row sits on the
/// page's own white directly under the ruled header. Both documents ask this
/// rather than each deciding what "every other" counts from — off by one here
/// and the two documents stripe opposite rows.
pub fn row_is_shaded(index: usize) -> bool {
    index % 2 == 1
}

/// How much of a document's content width the logo may occupy.
///
/// A fraction rather than a length, because the page measures in `rem` against
/// its body width and the PDF in millimetres against its printable width; the
/// one thing that has to match is how large the mark reads, which is its share
/// of the measure it sits in. A letterhead is a masthead, not a banner.
pub const LOGO_WIDTH_FRACTION: f32 = 0.20;

/// And of that width for its height, which is what stops a tall mark from
/// growing down the page when the width cap never binds.
pub const LOGO_HEIGHT_FRACTION: f32 = 0.056;

/// One line of the money block, in the order both documents print them.
pub struct MoneyLine {
    pub label: &'static str,
    pub amount: f64,
    /// The one line a reader's eye should land on: the **last** one, which is
    /// whatever this invoice actually leaves owing — the total on an unpaid
    /// invoice, the balance once something has been paid, the credit when
    /// somebody has paid too much.
    ///
    /// Exactly one line carries it, and both documents emphasise it the same
    /// way: by **weight alone**, at the same size as every other money line.
    /// Two lines set large and bold with a small plain one between them reads
    /// as two headlines and a whisper; one column of figures with the bottom
    /// line picked out reads as a bill.
    pub emphasis: bool,
    /// A row the payment block introduced.
    ///
    /// These are new to *both* documents, so both render them the same way —
    /// `USD 60.00` — rather than one of them inheriting the PDF's older
    /// `$`-prefixed style, which cannot say which currency it means. The
    /// pre-existing Subtotal/Tax/Total rows keep each document's own
    /// convention; reconciling those is TASK-87's.
    pub payment_row: bool,
}

/// The figures both documents draw, and the rules about which of them appear.
pub struct MoneySummary {
    pub subtotal: f64,
    pub tax: f64,
    pub total: f64,
    pub paid: f64,
    /// What is still owed. Never negative: an overpayment is a `credit`.
    pub balance: f64,
    /// What was paid beyond the total, when anything was. Zero otherwise.
    pub credit: f64,
}

impl MoneySummary {
    pub fn of(invoice: &Invoice, paid: f64) -> Self {
        // The same question `refresh_status` asks, through the same function:
        // a document that disagreed would print a balance under a status that
        // says `paid`.
        let settled = is_settled(invoice.total, paid);
        let over = paid - invoice.total;
        Self {
            subtotal: invoice.subtotal,
            tax: invoice.tax,
            total: invoice.total,
            paid,
            balance: if settled { 0.0 } else { invoice.total - paid },
            credit: if over > CENT_SLACK { over } else { 0.0 },
        }
    }

    /// A line appears when it has something to say.
    ///
    /// The subtotal/tax rule is the one the PDF has always applied: a one-line
    /// invoice with no tax prints one figure. Paid and Balance appear together,
    /// because a Paid row with no balance beside it leaves the reader to do the
    /// subtraction. Credit appears only when someone has paid too much, which
    /// is a fact about money owed the other way and never a negative balance.
    pub fn lines(&self) -> Vec<MoneyLine> {
        let mut lines = Vec::with_capacity(6);
        let mut push = |label, amount, emphasis, payment_row| {
            lines.push(MoneyLine {
                label,
                amount,
                emphasis,
                payment_row,
            })
        };
        if self.tax != 0.0 {
            push("Subtotal", self.subtotal, false, false);
            push("Tax", self.tax, false, false);
        }
        push("Total", self.total, false, false);
        if self.paid > 0.0 {
            push("Paid", self.paid, false, true);
            push("Balance due", self.balance, false, true);
            if self.credit > 0.0 {
                push("Credit", self.credit, false, true);
            }
        }
        // The emphasis is positional rather than per-label: whichever line ends
        // the block is the one that says what is owed, and marking it here is
        // what stops the two renderers disagreeing about which that is.
        if let Some(last) = lines.last_mut() {
            last.emphasis = true;
        }
        lines
    }
}

/// A billing address as the lines it was typed as, blank ones dropped and the
/// block clamped to what a document can draw.
///
/// Both documents draw one line per row, so an address entered over two lines
/// stays two lines and an address entered with a trailing newline grows no
/// empty row. Beyond `MAX_ADDRESS_LINES` the block is cut and the cut is shown:
/// the PDF has no page-break logic under this loop, and silently dropping the
/// rest would leave the two documents saying different things.
pub fn address_lines(address: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = address
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(MAX_ADDRESS_LINES + 1)
        .collect();
    if lines.len() > MAX_ADDRESS_LINES {
        lines.truncate(MAX_ADDRESS_LINES - 1);
        lines.push(ADDRESS_TRUNCATED);
    }
    lines
}

/// The client's email as a document should print it, or nothing.
///
/// Blank-is-absent is a rule about the document, not about either renderer, so
/// it lives here beside `address_lines` — otherwise a client whose email is a
/// single space is a stray `<br>` on one document and a drawn empty row on the
/// other.
pub fn email_line(email: Option<&str>) -> Option<&str> {
    email.map(str::trim).filter(|e| !e.is_empty())
}

/// The operator's own payment instructions, as the lines they were typed as.
///
/// `address_lines` without the clamp, and deliberately so. An address is a
/// postal fact with a natural length; this is the operator's prose about their
/// own bank, and cutting it off after six lines with a `...` would be Nigel
/// editing a sentence about where money goes. Both documents draw every line —
/// the PDF through `table_row_wrapped`, which paginates.
pub fn payment_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

/// Who the invoice is from, as both documents draw it.
pub struct CompanyBlock<'a> {
    pub name: &'a str,
    pub address: Vec<&'a str>,
    pub phone: Option<&'a str>,
}

impl CompanyBlock<'_> {
    /// Nothing to draw. A From heading over an empty block is the failure mode
    /// this answers, on both documents at once.
    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.address.is_empty() && self.phone.is_none()
    }
}

/// The From block, decided once. The address goes through the same
/// `address_lines` a client's does — one clamp, one truncation marker, both
/// parties — and the phone through `email_line`'s trim-or-nothing rule.
pub fn company_block<'a>(name: &'a str, address: &'a str, phone: &'a str) -> CompanyBlock<'a> {
    CompanyBlock {
        name: name.trim(),
        address: address_lines(address),
        phone: email_line(Some(phone)),
    }
}

/// One label/value row of the invoice metadata column.
pub struct MetaRow {
    pub label: &'static str,
    pub value: String,
    /// The row a reader's eye should land on.
    pub emphasis: bool,
}

/// The metadata rows both documents print, in order. A row with nothing to say
/// is absent rather than empty.
pub fn meta_rows(invoice: &Invoice) -> Vec<MetaRow> {
    let mut rows = vec![
        MetaRow {
            label: "Invoice ID",
            value: invoice.number.to_string(),
            emphasis: true,
        },
        MetaRow {
            label: "Issue Date",
            value: invoice.issue_date.clone(),
            emphasis: false,
        },
    ];
    if let Some(value) = due_value(invoice) {
        rows.push(MetaRow {
            label: "Due Date",
            value,
            emphasis: false,
        });
    }
    rows
}

/// The due date as a document prints it, with its terms folded in when they fit
/// on the line.
///
/// Single-line terms read naturally in parentheses after the date, which is
/// what the reference does. A paragraph does not, so multi-line terms stay a
/// block — and the alternative, a character-count threshold, would be a hidden
/// rule nobody could predict.
pub fn due_value(invoice: &Invoice) -> Option<String> {
    let due = invoice
        .due_date
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())?;
    match folded_terms(invoice) {
        Some(terms) => Some(format!("{due} ({terms})")),
        None => Some(due.to_string()),
    }
}

/// The terms when they belong beside the due date: there is a due date, and the
/// terms are one line.
fn folded_terms(invoice: &Invoice) -> Option<&str> {
    invoice
        .due_date
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())?;
    let terms = trimmed_terms(invoice)?;
    (!terms.contains('\n')).then_some(terms)
}

fn trimmed_terms(invoice: &Invoice) -> Option<&str> {
    invoice
        .terms
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// The terms as their own block, and only when `due_value` did not already
/// print them — so nothing appears twice.
pub fn terms_block_text(invoice: &Invoice) -> Option<&str> {
    let terms = trimmed_terms(invoice)?;
    folded_terms(invoice).is_none().then_some(terms)
}

/// How large a stored logo may be, decoded.
///
/// Every byte is base64-inflated by a third into every email body and every
/// published object, and the page is the email.
pub const MAX_LOGO_BYTES: usize = 128 * 1024;

/// The image types a logo may be.
///
/// SVG is not among them: most mail clients will not render it, and printpdf
/// cannot embed it, so allowing it would buy a validation branch and two
/// documents that disagree.
const LOGO_MIMES: &[(&str, &[u8])] = &[
    (
        "image/png",
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
    ),
    ("image/jpeg", &[0xff, 0xd8, 0xff]),
];

/// A validated logo, carrying what each document needs from one parse.
pub struct Logo {
    pub mime: &'static str,
    /// The payload as stored, ready to go straight into an `<img src>`.
    pub base64: String,
    /// The image file itself, for the PDF to decode and embed.
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The stored `company_logo` value, checked end to end, or the reason it cannot
/// be used.
///
/// `Ok(None)` is an unset logo, which is not a failure — clearing the field is
/// how an operator removes one. Every other refusal names what was wrong, so
/// the settings screen that runs this before writing can say it.
///
/// The dimensions are read out of the file header rather than by decoding,
/// because this module has to validate a logo identically in a build with no
/// `pdf` feature, where the `image` crate does not exist.
pub fn parse_logo(data_uri: &str) -> Result<Option<Logo>> {
    let uri = data_uri.trim();
    if uri.is_empty() {
        return Ok(None);
    }

    let declared = uri
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(";base64,"))
        .ok_or_else(|| {
            NigelError::Invalid(
                "A logo must be a data: URI shaped like data:image/png;base64,<payload>.".into(),
            )
        })?;
    let (mime, payload) = declared;

    let (mime, magic) = LOGO_MIMES
        .iter()
        .find(|(known, _)| *known == mime)
        .ok_or_else(|| {
            NigelError::Invalid(format!(
                "A logo of type {mime} cannot be used. PNG (image/png) and JPEG (image/jpeg) are the accepted types."
            ))
        })?;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| NigelError::Invalid(format!("The logo's base64 payload is invalid: {e}")))?;

    if !bytes.starts_with(magic) {
        return Err(NigelError::Invalid(format!(
            "The logo says it is {mime}, but its contents are not a {}.",
            format_name(mime)
        )));
    }
    if bytes.len() > MAX_LOGO_BYTES {
        return Err(NigelError::Invalid(format!(
            "The logo is {} bytes; the limit is {MAX_LOGO_BYTES} bytes.",
            bytes.len()
        )));
    }

    if !ends_correctly(mime, &bytes) {
        return Err(NigelError::Invalid(format!(
            "The logo is an incomplete {}: the file ends before its end marker.",
            format_name(mime)
        )));
    }

    let (width, height) = image_dimensions(mime, &bytes).ok_or_else(|| {
        NigelError::Invalid(format!(
            "The logo's dimensions could not be read; it is not a usable {}.",
            format_name(mime)
        ))
    })?;

    Ok(Some(Logo {
        mime,
        base64: payload.to_string(),
        bytes,
        width,
        height,
    }))
}

/// Written by hand because the derived one would print 128 KiB of base64 and a
/// vector of bytes into any message that formats a `Logo`.
impl std::fmt::Debug for Logo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Logo({}, {}x{}, {} bytes)",
            self.mime,
            self.width,
            self.height,
            self.bytes.len()
        )
    }
}

fn format_name(mime: &str) -> &'static str {
    if mime == "image/jpeg" {
        "JPEG"
    } else {
        "PNG"
    }
}

/// Whether the file reaches its own terminator — a PNG's `IEND` chunk, a JPEG's
/// `FFD9` end-of-image marker.
///
/// A header is not a file. A logo cut off after its `IHDR` still answers "how
/// big am I", so the dimension check alone would accept it, the page would show
/// a broken `<img>` and the PDF beside it would draw the wordmark — the two
/// documents disagreeing about the same stored value. This is the completeness
/// half of that decision, and it is here rather than in a renderer because it
/// costs no decoder and so holds identically in a build with no `pdf` feature.
fn ends_correctly(mime: &str, bytes: &[u8]) -> bool {
    if mime == "image/jpeg" {
        return bytes.ends_with(&[0xff, 0xd9]);
    }
    // The last chunk of a PNG is `IEND` plus its four CRC bytes.
    bytes.len() >= 12 && bytes[bytes.len() - 8..bytes.len() - 4] == *b"IEND"
}

/// The pixel size in a PNG's `IHDR` or a JPEG's `SOFn` frame. `None` for
/// anything truncated, malformed or zero-sized.
fn image_dimensions(mime: &str, bytes: &[u8]) -> Option<(u32, u32)> {
    let (width, height) = if mime == "image/jpeg" {
        jpeg_dimensions(bytes)?
    } else {
        png_dimensions(bytes)?
    };
    (width > 0 && height > 0).then_some((width, height))
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    // The IHDR chunk is required to be the first one, at a fixed offset.
    let header = bytes.get(8..24)?;
    (&header[4..8] == b"IHDR").then(|| (be_u32(&header[8..12]), be_u32(&header[12..16])))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    // Walk the marker segments to the frame header, which is the only one that
    // carries the size. Everything before it is metadata of some kind.
    let mut at = 2;
    loop {
        // A marker is 0xFF followed by a non-zero, non-0xFF code; runs of 0xFF
        // are legal padding before it.
        while *bytes.get(at)? == 0xff && *bytes.get(at + 1)? == 0xff {
            at += 1;
        }
        if *bytes.get(at)? != 0xff {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        // Start-of-frame, in every coding this format has: baseline,
        // extended, progressive and lossless, arithmetic-coded or not. The
        // three excluded values in each run are DHT, JPG and DAC.
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            let frame = bytes.get(at + 5..at + 9)?;
            return Some((
                u16::from_be_bytes([frame[2], frame[3]]) as u32,
                u16::from_be_bytes([frame[0], frame[1]]) as u32,
            ));
        }
        // Standalone markers carry no length word to skip over.
        if matches!(marker, 0x01 | 0xd0..=0xd9) {
            at += 2;
            continue;
        }
        let length = u16::from_be_bytes([*bytes.get(at + 2)?, *bytes.get(at + 3)?]) as usize;
        if length < 2 {
            return None;
        }
        at += 2 + length;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Invoice;

    /// A 2x1 PNG, and the smallest thing that is genuinely one.
    const PNG_2X1: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, // signature
        0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D', b'R', // IHDR, 13 bytes
        0x00, 0x00, 0x00, 0x02, // width 2
        0x00, 0x00, 0x00, 0x01, // height 1
        0x08, 0x06, 0x00, 0x00, 0x00, // bit depth, colour type, etc
        0x00, 0x00, 0x00, 0x00, // crc, unchecked
        0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', // IEND, the file's end
        0xae, 0x42, 0x60, 0x82, // crc, unchecked
    ];

    /// A JPEG whose SOF0 frame declares 3x7, terminated by its EOI marker.
    /// Nothing decodes it; `parse_logo` reads the header, the size and the end,
    /// which is all it claims to do.
    fn jpeg_3x7() -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xe0]; // SOI + APP0
        bytes.extend_from_slice(&[0x00, 0x04, 0x00, 0x00]); // APP0 length 4
        bytes.extend_from_slice(&[0xff, 0xc0]); // SOF0
        bytes.extend_from_slice(&[0x00, 0x11, 0x08]); // length 17, precision 8
        bytes.extend_from_slice(&[0x00, 0x07]); // height 7
        bytes.extend_from_slice(&[0x00, 0x03]); // width 3
        bytes.extend_from_slice(&[0x03; 10]);
        bytes.extend_from_slice(&[0xff, 0xd9]); // EOI
        bytes
    }

    fn data_uri(mime: &str, bytes: &[u8]) -> String {
        use base64::Engine as _;
        format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    #[test]
    fn a_png_data_uri_parses() {
        let uri = data_uri("image/png", PNG_2X1);
        let logo = parse_logo(&uri).unwrap().expect("a logo");
        assert_eq!(logo.mime, "image/png");
        assert_eq!(logo.bytes, PNG_2X1);
        assert_eq!((logo.width, logo.height), (2, 1));
        assert_eq!(
            format!("data:image/png;base64,{}", logo.base64),
            uri,
            "the page's src is the value as stored"
        );
    }

    #[test]
    fn a_jpeg_data_uri_parses_with_its_dimensions() {
        let jpeg = jpeg_3x7();
        let logo = parse_logo(&data_uri("image/jpeg", &jpeg))
            .unwrap()
            .expect("a logo");
        assert_eq!(logo.mime, "image/jpeg");
        assert_eq!((logo.width, logo.height), (3, 7));
    }

    #[test]
    fn the_empty_string_is_no_logo_rather_than_an_error() {
        assert!(parse_logo("").unwrap().is_none());
        assert!(parse_logo("   \n ").unwrap().is_none());
    }

    #[test]
    fn a_declared_png_that_is_not_a_png_is_refused() {
        // Right prefix, valid base64, wrong magic bytes.
        let err = parse_logo(&data_uri("image/png", b"not a png at all"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("PNG"), "got: {err}");
    }

    #[test]
    fn an_svg_or_a_gif_data_uri_is_refused_by_name() {
        for mime in ["image/svg+xml", "image/gif", "image/webp"] {
            let err = parse_logo(&data_uri(mime, PNG_2X1))
                .unwrap_err()
                .to_string();
            assert!(err.contains(mime), "{mime} unnamed in: {err}");
        }
    }

    #[test]
    fn something_that_is_not_a_data_uri_is_refused() {
        let err = parse_logo("https://example.test/logo.png")
            .unwrap_err()
            .to_string();
        assert!(err.contains("data:"), "got: {err}");
    }

    #[test]
    fn a_logo_over_the_cap_is_refused_with_its_size_in_the_message() {
        let mut big = PNG_2X1.to_vec();
        big.resize(MAX_LOGO_BYTES + 1, 0);
        let err = parse_logo(&data_uri("image/png", &big))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(&(MAX_LOGO_BYTES + 1).to_string()),
            "got: {err}"
        );
        assert!(err.contains("131072"), "the cap is named too: {err}");
    }

    #[test]
    fn a_payload_that_is_not_base64_is_refused() {
        let err = parse_logo("data:image/png;base64,!!!not base64!!!")
            .unwrap_err()
            .to_string();
        assert!(err.contains("base64"), "got: {err}");
    }

    #[test]
    fn a_png_whose_size_cannot_be_read_is_refused() {
        // Signature and end marker, no `IHDR` between them: complete as a file,
        // and still not something with a size.
        let mut headless = PNG_2X1[..8].to_vec();
        headless.extend_from_slice(&PNG_2X1[PNG_2X1.len() - 12..]);
        let err = parse_logo(&data_uri("image/png", &headless))
            .unwrap_err()
            .to_string();
        assert!(err.contains("dimensions"), "got: {err}");
    }

    /// A header is not a file. A logo cut off after its dimensions still
    /// answers "how big am I", and a page that trusted that would show a broken
    /// `<img>` while the PDF beside it drew the wordmark.
    #[test]
    fn a_png_that_stops_before_its_end_marker_is_refused() {
        let truncated = &PNG_2X1[..PNG_2X1.len() - 8];
        let err = parse_logo(&data_uri("image/png", truncated))
            .unwrap_err()
            .to_string();
        assert!(err.contains("incomplete"), "got: {err}");
    }

    #[test]
    fn a_jpeg_that_stops_before_its_end_marker_is_refused() {
        let jpeg = jpeg_3x7();
        let truncated = &jpeg[..jpeg.len() - 2];
        let err = parse_logo(&data_uri("image/jpeg", truncated))
            .unwrap_err()
            .to_string();
        assert!(err.contains("incomplete"), "got: {err}");
    }

    #[test]
    fn a_zero_by_zero_image_is_refused() {
        let mut flat = PNG_2X1.to_vec();
        flat[16..24].fill(0);
        let err = parse_logo(&data_uri("image/png", &flat))
            .unwrap_err()
            .to_string();
        assert!(err.contains("dimensions"), "got: {err}");
    }

    #[test]
    fn the_company_block_splits_its_address_the_way_a_client_address_is_split() {
        let block = company_block(
            "Bluepeak",
            "P.O. Box 1234\n\nSpringfield, CA 90001",
            " 619.555.0123 ",
        );
        assert_eq!(block.name, "Bluepeak");
        assert_eq!(
            block.address,
            vec!["P.O. Box 1234", "Springfield, CA 90001"]
        );
        assert_eq!(block.phone, Some("619.555.0123"));
        assert!(!block.is_empty());
    }

    #[test]
    fn an_unset_company_block_says_nothing_at_all() {
        let block = company_block("  ", "  \n ", "   ");
        assert!(block.address.is_empty());
        assert!(block.phone.is_none());
        assert!(block.is_empty());
    }

    #[test]
    fn a_long_company_address_is_clamped_the_same_way_a_client_address_is() {
        let typed = (1..=12)
            .map(|n| format!("Line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let block = company_block("Bluepeak", &typed, "");
        assert_eq!(block.address.len(), MAX_ADDRESS_LINES);
        assert_eq!(block.address[MAX_ADDRESS_LINES - 1], ADDRESS_TRUNCATED);
    }

    fn invoice_with(due: Option<&str>, terms: Option<&str>) -> Invoice {
        let mut inv = invoice(100.0, 0.0);
        inv.due_date = due.map(str::to_string);
        inv.terms = terms.map(str::to_string);
        inv
    }

    fn meta_labels(rows: &[MetaRow]) -> Vec<&'static str> {
        rows.iter().map(|r| r.label).collect()
    }

    #[test]
    fn the_metadata_rows_always_lead_with_the_invoice_id() {
        let rows = meta_rows(&invoice_with(None, None));
        assert_eq!(meta_labels(&rows), vec!["Invoice ID", "Issue Date"]);
        assert_eq!(rows[0].value, "1248");
        assert!(rows[0].emphasis, "the number is what a client quotes back");
        assert!(!rows[1].emphasis);
    }

    #[test]
    fn a_due_date_brings_its_row_and_a_missing_one_brings_nothing() {
        let rows = meta_rows(&invoice_with(Some("2026-09-05"), None));
        assert_eq!(
            meta_labels(&rows),
            vec!["Invoice ID", "Issue Date", "Due Date"]
        );
        assert_eq!(rows[2].value, "2026-09-05");
        assert!(!meta_labels(&meta_rows(&invoice_with(None, None))).contains(&"Due Date"));
    }

    #[test]
    fn single_line_terms_ride_beside_the_due_date() {
        assert_eq!(
            due_value(&invoice_with(Some("2026-09-05"), Some(" Net 30 "))).unwrap(),
            "2026-09-05 (Net 30)"
        );
    }

    #[test]
    fn multi_line_terms_stay_a_block_rather_than_a_parenthetical() {
        let inv = invoice_with(
            Some("2026-09-05"),
            Some("Net 30\nLate fees apply after 60 days."),
        );
        assert_eq!(due_value(&inv).unwrap(), "2026-09-05");
        assert!(
            terms_block_text(&inv).is_some(),
            "the paragraph has to land somewhere"
        );
    }

    #[test]
    fn folded_terms_do_not_also_print_as_a_block() {
        assert!(terms_block_text(&invoice_with(Some("2026-09-05"), Some("Net 30"))).is_none());
    }

    #[test]
    fn terms_with_no_due_date_are_a_block() {
        assert_eq!(
            terms_block_text(&invoice_with(None, Some("Net 30"))),
            Some("Net 30")
        );
    }

    #[test]
    fn blank_terms_are_no_terms_at_all() {
        let inv = invoice_with(Some("2026-09-05"), Some("   "));
        assert_eq!(due_value(&inv).unwrap(), "2026-09-05");
        assert!(terms_block_text(&inv).is_none());
    }

    #[test]
    fn payment_instructions_split_into_the_lines_they_were_typed_as() {
        assert_eq!(
            payment_lines("  Wells Fargo  \n\n  Routing 121000248  "),
            vec!["Wells Fargo", "Routing 121000248"]
        );
        assert!(payment_lines("   ").is_empty());
        assert!(payment_lines("").is_empty());
    }

    /// An address is a postal fact with a natural length. Instructions are the
    /// operator's own prose about their own bank, and cutting them off would be
    /// Nigel editing a sentence about where money goes.
    #[test]
    fn payment_instructions_are_never_clamped_the_way_an_address_is() {
        let typed = (1..=20)
            .map(|n| format!("Line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = payment_lines(&typed);
        assert_eq!(lines.len(), 20);
        assert_eq!(lines[19], "Line 20");
        assert!(!lines.contains(&ADDRESS_TRUNCATED));
    }

    fn invoice(total: f64, tax: f64) -> Invoice {
        Invoice {
            id: 1,
            number: 1248,
            client_id: 1,
            issue_date: "2026-08-04".into(),
            due_date: None,
            status: "sent".into(),
            currency: "USD".into(),
            subtotal: total - tax,
            tax,
            total,
            notes: None,
            terms: None,
            token: "t".into(),
            stripe_payment_link_id: None,
            stripe_payment_link_url: None,
            published_at: None,
            voided_at: None,
        }
    }

    fn labels(lines: &[MoneyLine]) -> Vec<&'static str> {
        lines.iter().map(|l| l.label).collect()
    }

    #[test]
    fn an_untaxed_unpaid_invoice_prints_one_line() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 0.0).lines();
        assert_eq!(labels(&lines), vec!["Total"]);
        assert!(lines[0].emphasis);
    }

    #[test]
    fn tax_brings_the_subtotal_with_it() {
        let lines = MoneySummary::of(&invoice(108.25, 8.25), 0.0).lines();
        assert_eq!(labels(&lines), vec!["Subtotal", "Tax", "Total"]);
        assert_eq!(lines[0].amount, 100.0);
        assert_eq!(lines[1].amount, 8.25);
        assert!(!lines[0].emphasis && !lines[1].emphasis);
    }

    #[test]
    fn a_payment_brings_paid_and_balance() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 40.0).lines();
        assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
        assert_eq!(lines[2].amount, 60.0);
        assert!(lines[2].emphasis, "the balance is what a client looks for");
    }

    /// The block is a short column of figures with one line emphasised, not two
    /// headlines with a whisper between them. The line that shouts is the last
    /// one — what is actually owed — whichever line that turns out to be.
    #[test]
    fn only_the_bottom_line_of_the_money_block_is_emphasised() {
        for (total, tax, paid) in [
            (100.0, 0.0, 0.0),   // Total alone
            (108.25, 8.25, 0.0), // Subtotal, Tax, Total
            (100.0, 0.0, 40.0),  // Total, Paid, Balance due
            (100.0, 0.0, 140.0), // Total, Paid, Balance due, Credit
        ] {
            let lines = MoneySummary::of(&invoice(total, tax), paid).lines();
            let emphasised: Vec<&str> = lines
                .iter()
                .filter(|l| l.emphasis)
                .map(|l| l.label)
                .collect();
            assert_eq!(
                emphasised,
                vec![lines.last().expect("a money block is never empty").label],
                "exactly the last line, for {:?}",
                labels(&lines)
            );
        }
    }

    #[test]
    fn a_settled_invoice_shows_a_zero_balance_rather_than_hiding_it() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 100.0).lines();
        assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
        assert_eq!(lines[2].amount, 0.0);
    }

    /// `f64` subtraction leaves a sliver behind; a document must never print
    /// `-0.00` next to "Balance due".
    #[test]
    fn a_balance_within_half_a_cent_of_zero_is_zero() {
        let summary = MoneySummary::of(&invoice(100.0, 0.0), 100.001);
        assert_eq!(summary.balance, 0.0);
        assert!(!summary.balance.is_sign_negative());
    }

    /// An overpaid invoice owes nothing; the excess is money going the other
    /// way, and a negative "Balance due" is not what that is.
    #[test]
    fn an_overpayment_reads_as_a_credit_rather_than_a_negative_balance() {
        let summary = MoneySummary::of(&invoice(100.0, 0.0), 130.0);
        let lines = summary.lines();

        assert_eq!(
            labels(&lines),
            vec!["Total", "Paid", "Balance due", "Credit"]
        );
        let due = lines.iter().find(|l| l.label == "Balance due").unwrap();
        assert_eq!(due.amount, 0.0);
        assert!(!due.amount.is_sign_negative(), "never a negative due");
        let credit = lines.iter().find(|l| l.label == "Credit").unwrap();
        assert_eq!(credit.amount, 30.0);
        assert_eq!(summary.credit, 30.0);
    }

    #[test]
    fn an_invoice_paid_to_the_penny_has_no_credit_line() {
        let lines = MoneySummary::of(&invoice(100.0, 0.0), 100.0).lines();
        assert_eq!(labels(&lines), vec!["Total", "Paid", "Balance due"]);
    }

    /// The document and `refresh_status` must answer "is this settled?" the same
    /// way, or a page whose status says `paid` prints a balance under it.
    #[test]
    fn the_settled_test_is_exactly_the_one_refresh_status_uses() {
        // Paid to within exactly half a cent: `is_settled` is inclusive at the
        // edge, so this invoice is `paid` and its document must agree.
        let total = 100.0;
        let paid = total - CENT_SLACK;
        assert!(
            crate::invoicing::invoices::is_settled(total, paid),
            "the fixture has to sit on the boundary"
        );

        let summary = MoneySummary::of(&invoice(total, 0.0), paid);
        assert_eq!(summary.balance, 0.0);
        let lines = summary.lines();
        let due = lines.iter().find(|l| l.label == "Balance due").unwrap();
        assert_eq!(
            format!("{:.2}", due.amount),
            "0.00",
            "a settled invoice may not print a balance"
        );
    }

    /// The rows the payment block introduced are new to both documents, so both
    /// render them the same way rather than one inheriting an older convention.
    #[test]
    fn the_payment_rows_are_flagged_for_identical_rendering() {
        let lines = MoneySummary::of(&invoice(108.25, 8.25), 130.0).lines();
        for line in &lines {
            let expected = matches!(line.label, "Paid" | "Balance due" | "Credit");
            assert_eq!(
                line.payment_row, expected,
                "{} is flagged wrong",
                line.label
            );
        }
    }

    #[test]
    fn an_address_is_split_into_the_lines_it_was_typed_as() {
        assert_eq!(
            address_lines("123 Main St\nSpringfield, IL"),
            vec!["123 Main St", "Springfield, IL"]
        );
        assert!(
            address_lines("  \n \n").is_empty(),
            "blank lines say nothing"
        );
        assert!(address_lines("").is_empty());
        assert_eq!(
            address_lines("  123 Main St  \r\n\n  Springfield  "),
            vec!["123 Main St", "Springfield"],
            "a blank line in the middle is not a line"
        );
    }

    #[test]
    fn an_email_prints_only_when_there_is_one() {
        assert_eq!(email_line(Some("  ap@acme.test  ")), Some("ap@acme.test"));
        assert_eq!(email_line(Some("   ")), None);
        assert_eq!(email_line(Some("")), None);
        assert_eq!(email_line(None), None);
    }

    /// A client block that runs off the bottom of the page is never wanted on
    /// an invoice, and the two documents have to clamp identically or the
    /// parity this module exists for is gone.
    #[test]
    fn a_long_address_is_clamped_with_something_that_says_so() {
        let typed = (1..=12)
            .map(|n| format!("Line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = address_lines(&typed);

        assert_eq!(lines.len(), MAX_ADDRESS_LINES);
        assert_eq!(lines[0], "Line 1");
        assert_eq!(
            lines[MAX_ADDRESS_LINES - 1],
            ADDRESS_TRUNCATED,
            "the reader is told the block was cut, not left to wonder"
        );
        // An address that fits is untouched, indicator and all.
        let short = address_lines("A\nB");
        assert_eq!(short, vec!["A", "B"]);
        assert!(!short.contains(&ADDRESS_TRUNCATED));
    }

    /// Exactly at the limit is not truncation.
    #[test]
    fn an_address_that_just_fits_keeps_every_line() {
        let typed = (1..=MAX_ADDRESS_LINES)
            .map(|n| format!("Line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = address_lines(&typed);
        assert_eq!(lines.len(), MAX_ADDRESS_LINES);
        assert_eq!(
            lines[MAX_ADDRESS_LINES - 1],
            format!("Line {MAX_ADDRESS_LINES}")
        );
    }
}
