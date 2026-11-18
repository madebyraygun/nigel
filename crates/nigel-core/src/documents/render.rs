//! Recipient pages, notices and email text. Preview and send both come through
//! here, so what an operator previews is what a client receives. Every value
//! interpolated into HTML is escaped with the invoicing `esc`; the inline script is
//! fixed text that reads its values from `data-*` attributes.
use super::model::{RecipientRole, SignatureRole};
use crate::invoicing::render_html::esc;

pub struct PageContext<'a> {
    pub company: &'a str,
    pub client_name: &'a str,
    pub kind: &'a str,
    pub title: &'a str,
    pub token: &'a str,
    pub version: i64,
    pub checksum: &'a str,
    pub pdf_href: &'a str,
    /// The letterhead logo's `<img src>`: the address invoices published it
    /// at, or the image inline as a `data:` URI. `None` draws no image.
    pub logo_src: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct PageRecipient<'a> {
    pub token: &'a str,
    pub role: RecipientRole,
    pub name: &'a str,
}

/// One change request as the page shows it. `at` is the stored timestamp: a
/// UTC `YYYY-MM-DDTHH:MM:SSZ` for an online response, a bare date for one
/// recorded by hand.
#[derive(Debug, Clone)]
pub struct RequestedChange {
    pub name: String,
    pub note: String,
    pub at: String,
}

/// One signature as a page states it: who, the role they signed in when they
/// gave one, and the date.
#[derive(Debug, Clone, Copy)]
pub struct Stamp<'a> {
    pub name: &'a str,
    pub title: Option<&'a str>,
    pub date: &'a str,
}

impl Stamp<'_> {
    /// [`signed_by`], escaped.
    fn by(&self, on_behalf_of: Option<&str>) -> String {
        esc(&signed_by(self.name, self.title, on_behalf_of, self.date))
    }
}

/// Who signed and when, as plain text: `Pat Example on 2026-10-06`, or with a
/// title or the organization they signed for, each set off by commas:
/// `Pat Example, Director of Operations, on behalf of Cedar Systems, on
/// 2026-10-06`.
fn signed_by(name: &str, title: Option<&str>, on_behalf_of: Option<&str>, date: &str) -> String {
    let title = title.filter(|t| !t.is_empty());
    let on_behalf_of = on_behalf_of.filter(|c| !c.is_empty());
    let mut who = name.to_string();
    if let Some(title) = title {
        who.push_str(&format!(", {title}"));
    }
    if let Some(client) = on_behalf_of {
        who.push_str(&format!(", on behalf of {client}"));
    }
    if title.is_some() || on_behalf_of.is_some() {
        format!("{who}, on {date}")
    } else {
        format!("{who} on {date}")
    }
}

/// What an executed page links beside the PDF, each relative to the page and
/// set only once that object is published.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExecutedLinks<'a> {
    /// The signed PDF with the redacted signing record appended.
    pub executed_copy: Option<&'a str>,
    pub signing_record: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub enum PageState<'a> {
    Open {
        response_url: Option<&'a str>,
    },
    Revising,
    /// An earlier version's page once `latest` has been sent. The newer
    /// version's recipients can differ, so the page names it and links nothing.
    Superseded {
        latest: i64,
    },
    ChangesRequested {
        requests: &'a [RequestedChange],
    },
    Accepted(Stamp<'a>),
    Executed {
        client: Stamp<'a>,
        countersign: Stamp<'a>,
        links: ExecutedLinks<'a>,
    },
    Declined,
}

const REPLY_BY_EMAIL: &str = "To respond, reply to the email this link came in.";

const SCRIPT: &str = r#"document.querySelectorAll('[data-actions]').forEach(function (bar) { bar.hidden = false; });
document.querySelectorAll('button[aria-controls]').forEach(function (toggle, _, toggles) {
  toggle.addEventListener('click', function () {
    var open = toggle.getAttribute('aria-expanded') !== 'true';
    toggles.forEach(function (t) {
      var show = open && t === toggle;
      t.setAttribute('aria-expanded', String(show));
      document.getElementById(t.getAttribute('aria-controls')).hidden = !show;
    });
    if (open) { var field = document.getElementById(toggle.getAttribute('aria-controls')).querySelector('input, textarea');
                if (field) { field.focus(); } }
  });
});
document.querySelectorAll('form[data-endpoint]').forEach(function (form) {
  form.addEventListener('submit', function (event) {
    event.preventDefault();
    var d = form.dataset, f = new FormData(form), status = form.querySelector('[data-status]');
    var body = { token: d.token, recipientToken: d.recipient, version: Number(d.version),
                 checksum: d.checksum, action: d.action };
    if (d.action === 'accept') { body.typedName = f.get('typedName'); body.consent = f.get('consent') === 'on';
                                 var title = String(f.get('title') || '').trim(); if (title) { body.title = title; } }
    else { body.note = f.get('note'); }
    form.querySelectorAll('button').forEach(function (b) { b.disabled = true; });
    fetch(d.endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
      .then(function (r) { return r.json().catch(function () { return {}; }).then(function (j) { return { ok: r.ok, j: j }; }); })
      .then(function (res) {
        if (res.ok) { document.querySelectorAll('form[data-endpoint], [data-actions]').forEach(function (x) { x.hidden = true; });
                      var received = document.getElementById('received');
                      received.textContent = received.dataset[d.action === 'accept' ? 'accept' : 'requestChanges'] || received.textContent;
                      received.hidden = false;
                      if (received.focus) { received.focus(); } }
        else { status.textContent = (res.j && res.j.message) || 'This response could not be recorded.';
               form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); }
      })
      .catch(function () { status.textContent = 'This response could not be sent. Check your connection and try again.';
                           form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); });
  });
});"#;

const STYLE: &str = "*,*::before,*::after{box-sizing:border-box}[hidden]{display:none!important}\
body{font-family:system-ui,-apple-system,'Segoe UI',sans-serif;max-width:52rem;margin:2.5rem auto 4rem;padding:0 1.25rem;color:#111;line-height:1.5;background:#fff}\
:focus-visible{outline:2px solid #111;outline-offset:2px}\
.letterhead{display:flex;align-items:center;justify-content:space-between;gap:1rem;padding-bottom:1.25rem;border-bottom:1px solid #d4d4d4;margin-bottom:2.25rem}\
.logo{max-width:8.8rem;max-height:2.5rem}\
.company,.eyebrow{margin:0;font-size:.75rem;text-transform:uppercase;letter-spacing:.06em;color:#5f5f5f}\
.eyebrow{margin-bottom:.35rem}\
h1{margin:0 0 1.25rem;font-size:1.75rem;line-height:1.2;letter-spacing:-.01em}\
table.meta{border-collapse:collapse;margin-bottom:2rem;font-size:.9375rem}\
table.meta th,table.meta td{padding:.2rem 1.5rem .2rem 0;text-align:left;vertical-align:baseline;font-weight:400}\
table.meta th{color:#5f5f5f;white-space:nowrap}\
.mono{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;font-size:.75rem;word-break:break-all;color:#5f5f5f}\
.doc{margin:0 0 1rem;border:1px solid #d4d4d4;border-radius:.5rem;overflow:hidden;background:#f4f4f4}\
.doc object{display:block;width:100%;height:75vh}\
.doc object p{padding:2rem;text-align:center}\
.download{display:flex;flex-wrap:wrap;gap:.5rem;margin:0 0 2.25rem}\
.download .button{min-height:2.25rem;padding:.4rem .9rem;font-size:.875rem}\
.respond{border-top:1px solid #d4d4d4;padding-top:2rem}\
.actions{display:flex;flex-wrap:wrap;gap:.75rem}\
.toggle{white-space:nowrap}.toggle::after{content:'';flex:none;width:.45em;height:.45em;margin-left:.65em;border:solid currentColor;border-width:0 2px 2px 0;transform:translateY(-.15em) rotate(45deg)}\
.toggle[aria-expanded=true]::after{transform:translateY(.15em) rotate(-135deg)}\
.card{max-width:34rem;margin-top:1.25rem;border:1px solid #d4d4d4;border-radius:.5rem;padding:1.5rem;background:#fafafa}\
label{display:block;margin:0 0 1rem;font-size:.9rem;color:#333}\
input[type=text],textarea{display:block;width:100%;margin-top:.35rem;padding:.6rem .75rem;font:inherit;color:#111;border:1px solid #8a8a8a;border-radius:.4rem;background:#fff}\
input[type=text]:focus,textarea:focus{outline:2px solid #111;outline-offset:1px}\
textarea{resize:vertical;min-height:7rem}\
.optional{color:#5f5f5f}\
.behalf{margin:-.25rem 0 1rem;font-size:.9rem;color:#333}\
.note{font-size:.8125rem;color:#444;overflow-wrap:anywhere;margin:0 0 1rem;padding-top:1rem;border-top:1px solid #e2e2e2}\
.check{display:flex;gap:.6rem;align-items:flex-start;margin-bottom:1.25rem}.check input{margin:.25rem 0 0;width:1rem;height:1rem;flex:none;accent-color:#111}\
.button,button{display:inline-flex;align-items:center;justify-content:center;min-height:2.75rem;padding:.6rem 1.25rem;font:inherit;font-weight:600;line-height:1.2;border-radius:.4rem;border:1px solid #111;cursor:pointer;text-decoration:none}\
.primary{background:#111;color:#fff}.secondary{background:#fff;color:#111}\
.primary:hover{background:#333}.secondary:hover{background:#f0f0f0}\
button:disabled{opacity:.5;cursor:default}\
.status{margin:.75rem 0 0;font-size:.875rem;color:#a40000}.status:empty{display:none}\
.notice{margin:0;padding:.9rem 1.1rem;border-radius:.5rem;background:#f4f4f4;border:1px solid #d4d4d4}\
.notice p{margin:0}.notice p+p{margin-top:.6rem}.request{white-space:pre-wrap;overflow-wrap:anywhere}\
.success{background:#eef7ee;border-color:#9cc79c}\
@media (max-width:36rem){body{margin-top:1.5rem}h1{font-size:1.4rem}table.meta th{padding-right:1rem}.doc object{height:60vh}.actions button{flex:1 1 auto}.card{padding:1.1rem}}\
@media print{.download,.doc,.actions,.forms,#received{display:none}.respond{border:0;padding:0}}";

pub fn relative_pdf_href(version: i64) -> String {
    format!("../v{version}/document.pdf")
}

pub fn relative_signing_record_href(version: i64) -> String {
    format!("../v{version}/signing-record.pdf")
}

pub fn relative_executed_copy_href(version: i64) -> String {
    format!("../v{version}/executed.pdf")
}

fn page_shell(title: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<meta name=\"robots\" content=\"noindex\"><title>{}</title><style>{STYLE}</style></head>\
<body>\n{body}\n</body></html>\n",
        esc(title)
    )
}

fn data_attrs(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    endpoint: &str,
    action: &str,
) -> String {
    format!(
        "data-endpoint=\"{}\" data-token=\"{}\" data-recipient=\"{}\" data-version=\"{}\" data-checksum=\"{}\" data-action=\"{}\"",
        esc(endpoint),
        esc(ctx.token),
        esc(recipient.token),
        ctx.version,
        esc(ctx.checksum),
        action
    )
}

const SIGNATURE_INTENT: &str = "Your typed name is your electronic signature on this document.";

fn signing_paragraph(checksum: &str) -> String {
    format!(
        "<p class=\"note\">Accepting records your typed name, the time, your IP address and browser against this exact file (checksum {checksum}). {SIGNATURE_INTENT}</p>"
    )
}

/// The line under the signature fields saying whom the signer signs for.
fn on_behalf_line(client: &str) -> String {
    if client.is_empty() {
        return String::new();
    }
    format!(
        "<p class=\"behalf\">Signing on behalf of <strong>{}</strong></p>",
        esc(client)
    )
}

fn forms(ctx: &PageContext<'_>, recipient: &PageRecipient<'_>, endpoint: &str) -> String {
    let signer = recipient.role == RecipientRole::Signer;
    let mut out = String::from("<div class=\"actions\" data-actions hidden>");
    if signer {
        out.push_str(
            "<button type=\"button\" class=\"primary toggle\" aria-expanded=\"false\" aria-controls=\"respond-accept\">Accept</button>",
        );
    }
    out.push_str(
        "<button type=\"button\" class=\"secondary toggle\" aria-expanded=\"false\" aria-controls=\"respond-changes\">Request changes</button></div>\n\
<div class=\"forms\">\n",
    );
    if signer {
        out.push_str(&format!(
            "<form method=\"post\" hidden {} id=\"respond-accept\" aria-label=\"Accept\" class=\"card\">\
<label>Your full name <input type=\"text\" name=\"typedName\" required maxlength=\"200\" autocomplete=\"name\"></label>\
<label>Your title <span class=\"optional\">(optional)</span> <input type=\"text\" name=\"title\" maxlength=\"200\" autocomplete=\"organization-title\"></label>\
{behalf}{signing}\
<label class=\"check\"><input type=\"checkbox\" name=\"consent\" required> I agree to sign electronically</label>\
<button type=\"submit\" class=\"primary\">Accept</button><p data-status role=\"status\" class=\"status\"></p></form>\n",
            data_attrs(ctx, recipient, endpoint, "accept"),
            behalf = on_behalf_line(ctx.client_name),
            signing = signing_paragraph(&esc(ctx.checksum)),
        ));
    }
    out.push_str(&format!(
        "<form method=\"post\" hidden {} id=\"respond-changes\" aria-label=\"Request changes\" class=\"card\">\
<label>What should change? <textarea name=\"note\" rows=\"5\" maxlength=\"4000\" required></textarea></label>\
<button type=\"submit\" class=\"secondary\">Request changes</button><p data-status role=\"status\" class=\"status\"></p></form>\n</div>\n\
<p id=\"received\" class=\"notice success\" hidden tabindex=\"-1\" data-accept=\"Thank you, your acceptance has been received.\" data-request-changes=\"Thank you, your request has been received.\">Thank you, your response has been received.</p>\n<script>{SCRIPT}</script>\n\
<noscript><p>{REPLY_BY_EMAIL}</p></noscript>\n",
        data_attrs(ctx, recipient, endpoint, "request_changes")
    ));
    out
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Rewrites each `<time data-local>` into the reader's own time zone. The
/// server-rendered text, in UTC, is what a reader without script sees.
const LOCAL_TIME_SCRIPT: &str = r#"document.querySelectorAll('time[data-local]').forEach(function (t) {
  var d = new Date(t.getAttribute('datetime'));
  if (isNaN(d)) return;
  t.textContent = d.toLocaleDateString(undefined, { month: 'long', day: 'numeric' }) + ' at ' +
    d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
});"#;

/// `October 9` for a date, and a `<time>` reading `October 9 at 8:44pm UTC`
/// for an online timestamp, which the page localizes. `None` when `at` is
/// neither, so the sentence drops its date rather than printing a raw value.
fn when(at: &str) -> Option<(String, bool)> {
    let month: usize = at.get(5..7)?.parse().ok()?;
    let day: u32 = at.get(8..10)?.parse().ok()?;
    let date = format!("{} {day}", MONTHS.get(month.checked_sub(1)?)?);
    if at.len() == 10 {
        return Some((date, false));
    }
    let hour: u32 = at.get(11..13)?.parse().ok()?;
    let minute = at.get(14..16)?;
    let (h12, half) = match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
    };
    Some((
        format!(
            "<time datetime=\"{}\" data-local>{date} at {h12}:{minute}{half} UTC</time>",
            esc(at)
        ),
        true,
    ))
}

fn changes_requested(requests: &[RequestedChange]) -> String {
    let mut out = String::from(
        "<div class=\"notice\"><p><strong>Changes have been requested. A new document will be sent.</strong></p>\n",
    );
    let mut timed = false;
    for request in requests {
        let on = match when(&request.at) {
            Some((text, is_time)) => {
                timed |= is_time;
                format!(" on {text}")
            }
            None => String::new(),
        };
        out.push_str(&format!(
            "<p>{} said <q class=\"request\">{}</q>{on}.</p>\n",
            esc(&request.name),
            esc(&request.note)
        ));
    }
    out.push_str("</div>\n");
    if timed {
        out.push_str(&format!("<script>{LOCAL_TIME_SCRIPT}</script>\n"));
    }
    out
}

fn state_section(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    state: &PageState<'_>,
) -> String {
    match state {
        PageState::Open {
            response_url: Some(url),
        } => forms(ctx, recipient, url),
        PageState::Open { response_url: None } => format!("<p class=\"notice\">{REPLY_BY_EMAIL}</p>\n"),
        PageState::Revising => {
            "<p class=\"notice\">This document is being revised. A new version will be sent to you.</p>\n".into()
        }
        PageState::Superseded { latest } => format!(
            "<p class=\"notice\">This version has been superseded by version {latest} and no longer takes responses.</p>\n"
        ),
        PageState::ChangesRequested { requests } => changes_requested(requests),
        PageState::Accepted(client) => format!(
            "<p class=\"notice success\">Accepted by {}.</p>\n",
            client.by(Some(ctx.client_name))
        ),
        PageState::Executed {
            client,
            countersign,
            ..
        } => format!(
            "<p class=\"notice success\">Accepted by {}.<br>Countersigned by {}.</p>\n",
            client.by(Some(ctx.client_name)),
            countersign.by(None)
        ),
        PageState::Declined => "<p class=\"notice\">This document was declined.</p>\n".into(),
    }
}

pub fn render_recipient_page(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    state: &PageState<'_>,
) -> String {
    let body = format!(
        "<header class=\"letterhead\">{logo}<p class=\"company\">{company}</p></header>\n\
<p class=\"eyebrow\">{kind} &middot; Version {version}</p>\n<h1>{title}</h1>\n\
<table class=\"meta\"><tr><th>Prepared for</th><td>{name}</td></tr>\
<tr><th>Client</th><td>{client}</td></tr>\
<tr><th>Checksum</th><td class=\"mono\">{checksum}</td></tr></table>\n\
<div class=\"doc\"><object data=\"{href}\" type=\"application/pdf\"><p><a href=\"{href}\">Download the PDF</a></p></object></div>\n\
<p class=\"download\">{executed}<a class=\"button secondary\" href=\"{href}\">Download the PDF</a>{record}</p>\n\
<section class=\"respond\">\n{section}</section>",
        company = esc(ctx.company),
        title = esc(ctx.title),
        kind = esc(ctx.kind),
        client = esc(ctx.client_name),
        version = ctx.version,
        logo = ctx
            .logo_src
            .map(|src| format!("<img class=\"logo\" src=\"{}\" alt=\"\">", esc(src)))
            .unwrap_or_default(),
        name = esc(recipient.name),
        href = esc(ctx.pdf_href),
        executed = match state {
            PageState::Executed {
                links:
                    ExecutedLinks {
                        executed_copy: Some(copy),
                        ..
                    },
                ..
            } => format!(
                "<a class=\"button primary\" href=\"{}\">Download the executed copy</a>",
                esc(copy)
            ),
            _ => String::new(),
        },
        record = match state {
            PageState::Executed {
                links:
                    ExecutedLinks {
                        signing_record: Some(record),
                        ..
                    },
                ..
            } => format!(
                "<a class=\"button secondary\" href=\"{}\">Download the signing record</a>",
                esc(record)
            ),
            _ => String::new(),
        },
        checksum = esc(ctx.checksum),
        section = state_section(ctx, recipient, state),
    );
    page_shell(ctx.title, &body)
}

pub fn render_document_pages(
    ctx: &PageContext<'_>,
    recipients: &[PageRecipient<'_>],
    state: &PageState<'_>,
) -> Vec<(String, String)> {
    recipients
        .iter()
        .map(|r| (r.token.to_string(), render_recipient_page(ctx, r, state)))
        .collect()
}

pub fn withdrawn_page_html(company: &str, title: &str) -> String {
    page_shell(
        title,
        &format!(
            "<header><p class=\"meta\">{}</p><h1>{}</h1></header>\n<p>This document has been withdrawn.</p>\n",
            esc(company),
            esc(title)
        ),
    )
}

pub fn email_subject(company: &str, title: &str, role: RecipientRole) -> String {
    let ask = match role {
        RecipientRole::Signer => "Please review and sign",
        RecipientRole::Collaborator => "Please review",
    };
    if company.is_empty() {
        format!("{ask}: {title}")
    } else {
        format!("{ask}: {title} for {company}")
    }
}

pub fn render_document_email_text(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    url: &str,
) -> String {
    let ask = match recipient.role {
        RecipientRole::Signer => "Please review the attached document and respond using the link below.",
        RecipientRole::Collaborator => "The attached document is shared for your review. You can request changes using the link below.",
    };
    format!(
        "Hello {name},\n\n{ask}\n\n{title} (version {version})\n{url}\n",
        name = recipient.name,
        title = ctx.title,
        version = ctx.version,
    )
}

/// The confirmation's subject once a document is executed.
pub fn signed_subject(company: &str, title: &str) -> String {
    if company.is_empty() {
        format!("Signed: {title}")
    } else {
        format!("Signed: {title} for {company}")
    }
}

/// What a confirmation carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attached {
    /// One file: the signed PDF with the signing record appended.
    ExecutedCopy,
    /// The signed PDF and the signing record as two files.
    SignedAndRecord,
    /// The signed PDF alone, when no record could be made.
    SignedOnly,
}

/// One signature as the confirmation lists it: `Accepted by Pat Example,
/// Director of Operations, on behalf of Cedar Systems, on 2026-10-06.`
pub fn signed_by_line(
    role: SignatureRole,
    name: &str,
    title: Option<&str>,
    on_behalf_of: Option<&str>,
    date: &str,
) -> String {
    let verb = match role {
        SignatureRole::Client => "Accepted",
        SignatureRole::Countersign => "Countersigned",
    };
    format!("{verb} by {}.", signed_by(name, title, on_behalf_of, date))
}

/// The confirmation's plain-text body: a greeting, one sentence that the
/// document is fully signed and what is attached, the title and version, who
/// signed, and the recipient's page when it is published.
pub fn render_signed_email_text(
    name: &str,
    title: &str,
    version: i64,
    signed_by: &[String],
    url: Option<&str>,
    attached: Attached,
) -> String {
    let attached = match attached {
        Attached::ExecutedCopy => {
            "the executed copy, the signed document with its signing record at the end, is attached"
        }
        Attached::SignedAndRecord => "the signed copy and its signing record are attached",
        Attached::SignedOnly => "the signed copy is attached",
    };
    let link = url.map(|u| format!("{u}\n")).unwrap_or_default();
    let signed = signed_by
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    format!("Hello {name},\n\nThis document is now fully signed; {attached}.\n\n{title} (version {version})\n{signed}{link}")
}

pub fn attachment_name(title: &str, version: i64) -> String {
    let mut stem = String::new();
    let mut pending_dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !stem.is_empty() {
                stem.push('-');
            }
            pending_dash = false;
            stem.push(c);
        } else {
            pending_dash = true;
        }
    }
    stem.truncate(80);
    let stem = stem.trim_end_matches('-');
    let stem = if stem.is_empty() { "document" } else { stem };
    format!("{stem}-v{version}.pdf")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::model::RecipientRole;

    fn ctx() -> PageContext<'static> {
        PageContext {
            company: "Initech",
            client_name: "Cedar Systems",
            kind: "Proposal",
            title: "Website rebuild",
            token: "doc",
            version: 2,
            checksum: "sha256:ab",
            pdf_href: "../v2/document.pdf",
            logo_src: None,
        }
    }
    const SIGNER: PageRecipient<'static> = PageRecipient {
        token: "rs",
        role: RecipientRole::Signer,
        name: "Pat Example",
    };
    const COLLAB: PageRecipient<'static> = PageRecipient {
        token: "rc",
        role: RecipientRole::Collaborator,
        name: "Sam Example",
    };
    const URL: Option<&str> = Some("https://docs.example.test/d/respond");

    /// The pages the Worker's contract test runs the inline script on, so the
    /// body the page posts is checked against what `parseRequest` reads.
    /// `NIGEL_UPDATE_WORKER_FIXTURE=1` rewrites the checked-in copy.
    #[test]
    fn the_worker_contract_fixture_matches_the_rendered_pages() {
        let (doc, signer, collaborator) = (
            "0123456789abcdef0123456789abcdef",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
        let ctx = PageContext {
            token: doc,
            ..ctx()
        };
        let recipients = [
            PageRecipient {
                token: signer,
                ..SIGNER
            },
            PageRecipient {
                token: collaborator,
                ..COLLAB
            },
        ];
        let pages =
            render_document_pages(&ctx, &recipients, &PageState::Open { response_url: URL });

        let accept = pages[0].1.split("<form").nth(1).expect("the accept form");
        for attr in [
            format!("data-token=\"{doc}\""),
            format!("data-recipient=\"{signer}\""),
            "data-version=\"2\"".to_string(),
            "data-checksum=\"sha256:ab\"".to_string(),
            "data-action=\"accept\"".to_string(),
        ] {
            assert!(accept.contains(&attr), "{attr}");
        }

        let fixture = serde_json::json!({
            "token": doc,
            "version": ctx.version,
            "checksum": ctx.checksum,
            "recipients": recipients.iter().map(|r| serde_json::json!({
                "token": r.token, "role": r.role.as_str(), "name": r.name,
            })).collect::<Vec<_>>(),
            "pages": pages.iter().map(|(token, html)| serde_json::json!({
                "recipientToken": token, "html": html,
            })).collect::<Vec<_>>(),
        });
        let rendered = format!("{}\n", serde_json::to_string_pretty(&fixture).unwrap());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../workers/document-response/test/fixtures/page-contract.json");
        if std::env::var_os("NIGEL_UPDATE_WORKER_FIXTURE").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &rendered).unwrap();
        }
        let checked_in = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            checked_in == rendered,
            "{} is stale: rerun this test with NIGEL_UPDATE_WORKER_FIXTURE=1",
            path.display()
        );
    }

    #[test]
    fn a_signer_page_carries_both_forms_and_a_collaborator_page_one() {
        let signer = render_recipient_page(&ctx(), &SIGNER, &PageState::Open { response_url: URL });
        assert!(
            signer.contains("data-action=\"accept\"")
                && signer.contains("data-action=\"request_changes\"")
        );
        let collab = render_recipient_page(&ctx(), &COLLAB, &PageState::Open { response_url: URL });
        assert!(
            !collab.contains("data-action=\"accept\"")
                && collab.contains("data-action=\"request_changes\"")
        );
        for page in [&signer, &collab] {
            assert!(page.contains("<meta name=\"robots\" content=\"noindex\">"));
            assert!(page.contains("sha256:ab") && page.contains("../v2/document.pdf"));
            assert!(page.contains(
                "<noscript><p>To respond, reply to the email this link came in.</p></noscript>"
            ));
            assert_eq!(
                page.matches("<form method=\"post\" hidden data-endpoint=")
                    .count(),
                page.matches("<form").count()
            );
        }
        assert!(signer.contains("I agree to sign electronically"));
        assert!(signer.contains(
            "<label>Your title <span class=\"optional\">(optional)</span> <input type=\"text\" name=\"title\" maxlength=\"200\""
        ));
        assert!(!signer.contains("name=\"title\" required"));
        assert!(signer.contains(
            "<p class=\"behalf\">Signing on behalf of <strong>Cedar Systems</strong></p>"
        ));
        assert!(!collab.contains("on behalf of"));
        assert!(signer.contains(
            "against this exact file (checksum sha256:ab). Your typed name is your electronic signature on this document."
        ));
        assert!(!collab.contains("electronic signature"));
        assert_eq!(signer.matches("<form").count(), 2);

        let accept_form = signer
            .split("data-action=\"accept\"")
            .nth(1)
            .and_then(|rest| rest.split("</form>").next())
            .expect("the accept form");
        assert!(accept_form.contains("Your typed name is your electronic signature"));
        assert!(signer.contains(
            "<div class=\"actions\" data-actions hidden>\
<button type=\"button\" class=\"primary toggle\" aria-expanded=\"false\" aria-controls=\"respond-accept\">Accept</button>\
<button type=\"button\" class=\"secondary toggle\" aria-expanded=\"false\" aria-controls=\"respond-changes\">Request changes</button></div>"
        ));
        assert!(collab.contains(
            "<div class=\"actions\" data-actions hidden>\
<button type=\"button\" class=\"secondary toggle\" aria-expanded=\"false\" aria-controls=\"respond-changes\">Request changes</button></div>"
        ));
        assert!(!collab.contains("respond-accept"));
        for (page, ids) in [
            (&signer, &["respond-accept", "respond-changes"][..]),
            (&collab, &["respond-changes"][..]),
        ] {
            for id in ids {
                assert_eq!(page.matches(&format!("id=\"{id}\"")).count(), 1, "{id}");
            }
        }
    }

    #[test]
    fn a_stamped_page_names_each_signer_with_their_title() {
        let page = render_recipient_page(
            &ctx(),
            &COLLAB,
            &PageState::Executed {
                client: Stamp {
                    name: "Pat Example",
                    title: Some("Director <of> Operations"),
                    date: "2026-10-06",
                },
                countersign: Stamp {
                    name: "Sam Example",
                    title: None,
                    date: "2026-10-07",
                },
                links: ExecutedLinks::default(),
            },
        );
        assert!(page.contains(
            "Accepted by Pat Example, Director &lt;of&gt; Operations, on behalf of Cedar Systems, on 2026-10-06.<br>Countersigned by Sam Example on 2026-10-07."
        ));
    }

    #[test]
    fn without_a_response_endpoint_there_is_no_form() {
        let page = render_recipient_page(&ctx(), &SIGNER, &PageState::Open { response_url: None });
        assert!(!page.contains("<form"));
        assert!(page.contains("reply to the email this link came in"));
    }

    #[test]
    fn closed_states_carry_no_form() {
        for state in [
            PageState::Revising,
            PageState::Superseded { latest: 3 },
            PageState::ChangesRequested { requests: &[] },
            PageState::Declined,
            PageState::Accepted(Stamp {
                name: "Pat Example",
                title: None,
                date: "2026-10-06",
            }),
            PageState::Executed {
                client: Stamp {
                    name: "Pat Example",
                    title: None,
                    date: "2026-10-06",
                },
                countersign: Stamp {
                    name: "Sam Example",
                    title: None,
                    date: "2026-10-07",
                },
                links: ExecutedLinks::default(),
            },
        ] {
            let page = render_recipient_page(&ctx(), &SIGNER, &state);
            assert!(!page.contains("<form"), "{state:?}");
            assert!(!page.contains("data-actions"), "{state:?}");
            assert!(!page.contains("electronic signature"), "{state:?}");
        }
        let accepted = render_recipient_page(
            &ctx(),
            &SIGNER,
            &PageState::Accepted(Stamp {
                name: "Pat Example",
                title: None,
                date: "2026-10-06",
            }),
        );
        assert!(
            accepted.contains("Accepted by Pat Example, on behalf of Cedar Systems, on 2026-10-06")
        );
    }

    #[test]
    fn a_note_or_name_with_markup_is_escaped_on_the_page() {
        let evil = PageRecipient {
            name: "<img src=x onerror=alert(1)>",
            ..SIGNER
        };
        let page = render_recipient_page(
            &PageContext {
                title: "<script>x</script>",
                ..ctx()
            },
            &evil,
            &PageState::Accepted(Stamp {
                name: "<b>Pat</b>",
                title: None,
                date: "2026-10-06",
            }),
        );
        assert!(
            !page.contains("<img src=x") && !page.contains("<script>x") && !page.contains("<b>Pat")
        );
        assert!(page.contains("&lt;b&gt;Pat&lt;/b&gt;"));
    }

    #[test]
    fn attribute_values_cannot_break_out() {
        let ctx = PageContext {
            token: "a\"><script>1</script>",
            checksum: "x\" onload=\"y",
            ..ctx()
        };
        let page = render_recipient_page(
            &ctx,
            &SIGNER,
            &PageState::Open {
                response_url: Some("https://e.test/\"><i>"),
            },
        );
        assert!(
            !page.contains("<script>1") && !page.contains("<i>") && !page.contains("onload=\"y")
        );
    }

    #[test]
    fn subjects_and_attachment_names() {
        assert_eq!(
            email_subject("Initech", "Website rebuild", RecipientRole::Signer),
            "Please review and sign: Website rebuild for Initech"
        );
        assert_eq!(
            email_subject("", "Website rebuild", RecipientRole::Collaborator),
            "Please review: Website rebuild"
        );
        assert_eq!(
            attachment_name("Website rebuild / Phase 2", 2),
            "Website-rebuild-Phase-2-v2.pdf"
        );
        assert_eq!(attachment_name("—", 1), "document-v1.pdf");
        assert_eq!(
            attachment_name(&"a".repeat(200), 3),
            format!("{}-v3.pdf", "a".repeat(80))
        );
    }

    #[test]
    fn the_email_body_is_plain_text_with_the_personal_link() {
        let text = render_document_email_text(
            &ctx(),
            &SIGNER,
            "https://docs.example.test/d/doc/rs/index.html",
        );
        assert!(text.contains("https://docs.example.test/d/doc/rs/index.html"));
        assert!(!text.contains('<'));
        assert!(text.ends_with("/d/doc/rs/index.html\n"));
        assert!(!text.contains("electronic signature"));
    }

    #[test]
    fn a_collaborator_email_ends_with_the_link() {
        let text = render_document_email_text(
            &ctx(),
            &COLLAB,
            "https://docs.example.test/d/doc/rc/index.html",
        );
        assert!(text.ends_with("/d/doc/rc/index.html\n"));
    }

    #[test]
    fn one_page_per_recipient() {
        let pages = render_document_pages(&ctx(), &[SIGNER, COLLAB], &PageState::Revising);
        assert_eq!(
            pages.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
            ["rs", "rc"]
        );
        assert!(!pages[1].1.contains("\"rs\""));
        let notice = |state| render_recipient_page(&ctx(), &SIGNER, &state);
        assert!(notice(PageState::Revising)
            .contains("This document is being revised. A new version will be sent to you."));
        assert!(notice(PageState::Superseded { latest: 3 }).contains(
            "This version has been superseded by version 3 and no longer takes responses."
        ));
        let requests = [
            RequestedChange {
                name: "Pat Example".into(),
                note: "Strike paragraph 3 <and> add a cancellation clause.".into(),
                at: "2026-10-09T23:44:51Z".into(),
            },
            RequestedChange {
                name: "Sam Example".into(),
                note: "Also fix the dates.".into(),
                at: "2026-10-10".into(),
            },
        ];
        let changes = notice(PageState::ChangesRequested {
            requests: &requests,
        });
        assert!(changes.contains("Changes have been requested. A new document will be sent."));
        assert!(changes.contains(
            "<p>Pat Example said <q class=\"request\">Strike paragraph 3 &lt;and&gt; add a cancellation clause.</q> on <time datetime=\"2026-10-09T23:44:51Z\" data-local>October 9 at 11:44pm UTC</time>.</p>"
        ));
        assert!(changes.contains(
            "<p>Sam Example said <q class=\"request\">Also fix the dates.</q> on October 10.</p>"
        ));
        assert!(changes.contains("time[data-local]") && !changes.contains("<form"));
        assert!(notice(PageState::Declined).contains("This document was declined."));
        let executed = notice(PageState::Executed {
            client: Stamp {
                name: "Pat Example",
                title: None,
                date: "2026-10-06",
            },
            countersign: Stamp {
                name: "Sam Example",
                title: None,
                date: "2026-10-07",
            },
            links: ExecutedLinks::default(),
        });
        assert!(
            executed.contains("Accepted by Pat Example, on behalf of Cedar Systems, on 2026-10-06")
                && executed.contains("Countersigned by Sam Example on 2026-10-07")
        );
        assert!(!executed.contains("signing record"));
        assert!(withdrawn_page_html("Initech", "<b>x</b>").contains("noindex"));
        assert_eq!(relative_pdf_href(3), "../v3/document.pdf");
    }

    #[test]
    fn an_executed_page_offers_the_signing_record_beside_the_pdf_once_published() {
        let href = relative_signing_record_href(2);
        assert_eq!(href, "../v2/signing-record.pdf");
        let page = render_recipient_page(
            &ctx(),
            &COLLAB,
            &PageState::Executed {
                client: Stamp {
                    name: "Pat Example",
                    title: None,
                    date: "2026-10-06",
                },
                countersign: Stamp {
                    name: "Sam Example",
                    title: None,
                    date: "2026-10-07",
                },
                links: ExecutedLinks {
                    executed_copy: None,
                    signing_record: Some(&href),
                },
            },
        );
        assert!(page.contains(
            "<p class=\"download\"><a class=\"button secondary\" href=\"../v2/document.pdf\">Download the PDF</a>\
<a class=\"button secondary\" href=\"../v2/signing-record.pdf\">Download the signing record</a></p>"
        ));
        for state in [
            PageState::Open { response_url: URL },
            PageState::Accepted(Stamp {
                name: "Pat Example",
                title: None,
                date: "2026-10-06",
            }),
        ] {
            assert!(!render_recipient_page(&ctx(), &SIGNER, &state).contains("signing record"));
        }
    }

    #[test]
    fn a_comma_in_the_signer_name_does_not_move_the_date_comma() {
        assert_eq!(
            signed_by_line(
                SignatureRole::Countersign,
                "Example, Pat",
                None,
                None,
                "2026-10-07"
            ),
            "Countersigned by Example, Pat on 2026-10-07."
        );
        assert_eq!(
            Stamp {
                name: "Example, Pat",
                title: None,
                date: "2026-10-07",
            }
            .by(None),
            "Example, Pat on 2026-10-07"
        );
        assert_eq!(
            Stamp {
                name: "Example, Pat",
                title: Some("R&D"),
                date: "2026-10-07",
            }
            .by(None),
            "Example, Pat, R&amp;D, on 2026-10-07"
        );
    }

    #[test]
    fn the_signed_confirmation_names_the_document_and_the_link() {
        assert_eq!(
            signed_subject("Initech", "Website rebuild"),
            "Signed: Website rebuild for Initech"
        );
        assert_eq!(
            signed_subject("", "Website rebuild"),
            "Signed: Website rebuild"
        );
        assert_eq!(
            render_signed_email_text(
                "Pat Example",
                "Website rebuild",
                2,
                &[
                    signed_by_line(
                        SignatureRole::Client,
                        "Pat Example",
                        Some("Director of Operations"),
                        Some("Cedar Systems"),
                        "2026-10-06"
                    ),
                    signed_by_line(
                        SignatureRole::Countersign,
                        "Sam Example",
                        None,
                        None,
                        "2026-10-07"
                    ),
                ],
                Some("https://docs.example.test/d/doc/rs/index.html"),
                Attached::SignedAndRecord
            ),
            "Hello Pat Example,\n\nThis document is now fully signed; the signed copy and its signing record are attached.\n\nWebsite rebuild (version 2)\nAccepted by Pat Example, Director of Operations, on behalf of Cedar Systems, on 2026-10-06.\nCountersigned by Sam Example on 2026-10-07.\nhttps://docs.example.test/d/doc/rs/index.html\n"
        );
        assert_eq!(
            render_signed_email_text(
                "Pat Example",
                "Website rebuild",
                2,
                &[],
                None,
                Attached::SignedOnly
            ),
            "Hello Pat Example,\n\nThis document is now fully signed; the signed copy is attached.\n\nWebsite rebuild (version 2)\n"
        );
    }
}
