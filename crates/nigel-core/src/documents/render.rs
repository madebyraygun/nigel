//! Recipient pages, notices and email text. Preview and send both come through
//! here, so what an operator previews is what a client receives. Every
//! interpolated value is escaped with the invoicing `esc`; the inline script is
//! fixed text that reads its values from `data-*` attributes.
use super::model::RecipientRole;
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
}

#[derive(Debug, Clone, Copy)]
pub struct PageRecipient<'a> {
    pub token: &'a str,
    pub role: RecipientRole,
    pub name: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub enum PageState<'a> {
    Open {
        response_url: Option<&'a str>,
    },
    Revising,
    ChangesRequested,
    Accepted {
        name: &'a str,
        date: &'a str,
    },
    Executed {
        client: (&'a str, &'a str),
        countersign: (&'a str, &'a str),
    },
    Declined,
}

const REPLY_BY_EMAIL: &str = "To respond, reply to the email this link came in.";

const SCRIPT: &str = r#"document.querySelectorAll('form[data-endpoint]').forEach(function (form) {
  form.addEventListener('submit', function (event) {
    event.preventDefault();
    var d = form.dataset, f = new FormData(form), status = form.querySelector('[data-status]');
    var body = { token: d.token, recipientToken: d.recipient, version: Number(d.version),
                 checksum: d.checksum, action: d.action };
    if (d.action === 'accept') { body.typedName = f.get('typedName'); body.consent = f.get('consent') === 'on'; }
    else { body.note = f.get('note'); }
    form.querySelectorAll('button').forEach(function (b) { b.disabled = true; });
    fetch(d.endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
      .then(function (r) { return r.json().catch(function () { return {}; }).then(function (j) { return { ok: r.ok, j: j }; }); })
      .then(function (res) {
        if (res.ok) { document.querySelectorAll('form[data-endpoint]').forEach(function (x) { x.hidden = true; });
                      document.getElementById('received').hidden = false; }
        else { status.textContent = (res.j && res.j.message) || 'This response could not be recorded.';
               form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); }
      })
      .catch(function () { status.textContent = 'This response could not be sent. Check your connection and try again.';
                           form.querySelectorAll('button').forEach(function (b) { b.disabled = false; }); });
  });
});"#;

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:52rem;margin:2rem auto;padding:0 1rem;color:#1a1a1a}\
object{width:100%;height:70vh;border:1px solid #ccc}label{display:block;margin:.5rem 0}\
textarea,input[type=text]{width:100%;box-sizing:border-box}.meta{color:#555;font-size:.9rem}\
fieldset{margin:1rem 0}";

pub fn relative_pdf_href(version: i64) -> String {
    format!("../v{version}/document.pdf")
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

fn forms(ctx: &PageContext<'_>, recipient: &PageRecipient<'_>, endpoint: &str) -> String {
    let mut out = String::new();
    if recipient.role == RecipientRole::Signer {
        out.push_str(&format!(
            "<form {}><h2>Accept</h2>\
<label>Your full name <input type=\"text\" name=\"typedName\" required maxlength=\"200\" autocomplete=\"name\"></label>\
<label><input type=\"checkbox\" name=\"consent\" required> I have read this document and agree to it.</label>\
<button type=\"submit\">Accept</button><p data-status role=\"status\"></p></form>\n",
            data_attrs(ctx, recipient, endpoint, "accept")
        ));
    }
    out.push_str(&format!(
        "<form {}><h2>Request changes</h2>\
<label>What should change? <textarea name=\"note\" rows=\"5\" maxlength=\"4000\" required></textarea></label>\
<button type=\"submit\">Request changes</button><p data-status role=\"status\"></p></form>\n\
<p id=\"received\" hidden>Received: thank you</p>\n<script>{SCRIPT}</script>\n\
<noscript><p>{REPLY_BY_EMAIL}</p></noscript>\n",
        data_attrs(ctx, recipient, endpoint, "request_changes")
    ));
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
        PageState::Open { response_url: None } => format!("<p>{REPLY_BY_EMAIL}</p>\n"),
        PageState::Revising => {
            "<p>This document is being revised. A new version will be sent to you.</p>\n".into()
        }
        PageState::ChangesRequested => {
            "<p>Changes requested: a revised version is on its way.</p>\n".into()
        }
        PageState::Accepted { name, date } => {
            format!("<p>Accepted by {} on {}.</p>\n", esc(name), esc(date))
        }
        PageState::Executed {
            client,
            countersign,
        } => format!(
            "<p>Accepted by {} on {}.</p>\n<p>Countersigned by {} on {}.</p>\n",
            esc(client.0),
            esc(client.1),
            esc(countersign.0),
            esc(countersign.1)
        ),
        PageState::Declined => "<p>This document was declined.</p>\n".into(),
    }
}

pub fn render_recipient_page(
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    state: &PageState<'_>,
) -> String {
    let body = format!(
        "<header><p class=\"meta\">{company}</p><h1>{title}</h1>\
<p class=\"meta\">{kind} for {client} &middot; Version {version}</p>\
<p class=\"meta\">Prepared for {name}</p></header>\n\
<object data=\"{href}\" type=\"application/pdf\"><p><a href=\"{href}\">Download the PDF</a></p></object>\n\
<p><a href=\"{href}\">Download the PDF</a></p>\n\
<p class=\"meta\">Checksum: {checksum}</p>\n\
<p>Accepting records your typed name, the time, your IP address and browser against this exact file (checksum {checksum}). It is a record of your agreement, not a certified electronic signature.</p>\n\
{section}",
        company = esc(ctx.company),
        title = esc(ctx.title),
        kind = esc(ctx.kind),
        client = esc(ctx.client_name),
        version = ctx.version,
        name = esc(recipient.name),
        href = esc(ctx.pdf_href),
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
    let prefix = if company.is_empty() {
        String::new()
    } else {
        format!("{company}: ")
    };
    let tail = match role {
        RecipientRole::Signer => "please review and sign",
        RecipientRole::Collaborator => "for your review",
    };
    format!("{prefix}{title}: {tail}")
}

pub fn render_document_email_text(
    company: &str,
    ctx: &PageContext<'_>,
    recipient: &PageRecipient<'_>,
    url: &str,
) -> String {
    let ask = match recipient.role {
        RecipientRole::Signer => "Please review the attached document and respond using the link below.",
        RecipientRole::Collaborator => "The attached document is shared for your review. You can request changes using the link below.",
    };
    let from = if company.is_empty() {
        String::new()
    } else {
        format!("\n{company}")
    };
    format!(
        "Hello {name},\n\n{ask}\n\n{title} (version {version})\n{url}\n\n\
Accepting records your typed name, the time, your IP address and browser against this exact file. It is a record of your agreement, not a certified electronic signature.\n{from}\n",
        name = recipient.name,
        title = ctx.title,
        version = ctx.version,
    )
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
            assert!(page.contains("not a certified electronic signature"));
        }
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
            PageState::ChangesRequested,
            PageState::Declined,
            PageState::Accepted {
                name: "Pat Example",
                date: "2026-10-06",
            },
            PageState::Executed {
                client: ("Pat Example", "2026-10-06"),
                countersign: ("Sam Example", "2026-10-07"),
            },
        ] {
            assert!(
                !render_recipient_page(&ctx(), &SIGNER, &state).contains("<form"),
                "{state:?}"
            );
        }
        let accepted = render_recipient_page(
            &ctx(),
            &SIGNER,
            &PageState::Accepted {
                name: "Pat Example",
                date: "2026-10-06",
            },
        );
        assert!(accepted.contains("Accepted by Pat Example on 2026-10-06"));
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
            &PageState::Accepted {
                name: "<b>Pat</b>",
                date: "2026-10-06",
            },
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
            "Initech: Website rebuild: please review and sign"
        );
        assert_eq!(
            email_subject("", "Website rebuild", RecipientRole::Collaborator),
            "Website rebuild: for your review"
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
            "Initech",
            &ctx(),
            &SIGNER,
            "https://docs.example.test/d/doc/rs/index.html",
        );
        assert!(text.contains("https://docs.example.test/d/doc/rs/index.html"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn one_page_per_recipient() {
        let pages = render_document_pages(&ctx(), &[SIGNER, COLLAB], &PageState::Revising);
        assert_eq!(
            pages.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
            ["rs", "rc"]
        );
        assert!(withdrawn_page_html("Initech", "<b>x</b>").contains("noindex"));
        assert_eq!(relative_pdf_href(3), "../v3/document.pdf");
    }
}
