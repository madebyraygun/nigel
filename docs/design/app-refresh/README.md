# Handoff: Nigel web app refresh

> **In this repository:** the spec and the screenshots. The clickable HTML prototypes it refers to (`prototypes/`) are kept outside the repository. The figures shown come from the demo books.
>
> **How it maps onto Nigel 2.0** (epic "Web app refresh" in the backlog): the web refresh ships for the browser, the iPad web app, and the Tauri shell on Windows and Linux. On the Mac, the Swift shell hides the web sidebar and status line and draws its own native sidebar, palette and settings window in this design's structure and look. Switching between sets of books is a separate task; until it lands, the breadcrumb, palette and status line show the one open book.

Repo: `madebyraygun/nigel` (`web/`). Covers the app shell, the book switcher, the status line, Dashboard, Register, Invoices (list and detail), Settings, Setup (onboarding) and the mobile drawer. **The app icon is not included**; it is still in design.

## Overview

The refresh polishes the existing Lit web app (`@nigel/ui`, `@nigel/theme`, `apps/app`) without changing its identity: IBM Plex Mono everywhere, the ink ramp on light and the pastel ramp on dark, and the rainbow gradient as an accent.

It also fixes one structural UX problem. The company name no longer sits in the sidebar's top-left corner; that corner now holds the hamburger. The company name moves into a breadcrumb above the content, and it is the entry point for switching between multiple sets of books.

Changes:

1. **Floating sidebar.** The sidebar is an inset, rounded panel (8px from the window edges), the same silhouette the native macOS Liquid Glass sidebar will have in Tauri. In the browser it is a flat, solid plane; in Tauri the native glass replaces it. Nav is grouped, and Settings is pinned at the bottom.
2. **Breadcrumb.** `[monogram] Book name ▾ / Screen` sits above the content, with a `/ #1254` level on invoice detail.
3. **⌘K palette.** Searches books and screens together. Clicking the breadcrumb opens it scoped to Books.
4. **Floating status line.** A second inset panel under the content: open book, folder path, lock state, review count, ⌘K hint and version.
5. **Screen polish.** Dashboard, Register, Invoices and Settings are reworked; Settings changes the most.

## About the design files

The files in `prototypes/` are **design references written in HTML**: clickable prototypes that show the intended look and behaviour. They are not production code. Rebuild them in Nigel's existing stack (Lit web components in `web/packages/ui`, tokens in `web/packages/theme`, screens in `web/apps/app/src/screens`), following the established patterns (`wc-*` elements, `NigelElement`, `css` tagged templates, `var(--color-*)` tokens).

To view them, serve the `prototypes/` folder over HTTP (for example `npx serve prototypes`) and open `Nigel App.dc.html`. Each file loads `support.js`, which is the prototype runtime and is not part of the design. Styles are inline in the markup; the logic sits in the `<script data-dc-script>` block at the bottom of each file.

## Fidelity

**High fidelity.** Colours, type sizes, spacing, radii and interactions are final. Where a value here differs from the current `@nigel/theme` tokens, take it as the new value. Mock data (book names, transactions, invoices, letterhead) is illustrative only.

---

## 1. App shell

Reference: `prototypes/Nigel App.dc.html`. The prototype is drawn at a fixed 1280 × 800 window with a fake 30px macOS title bar on top; the real Tauri window uses the standard title bar, so drop that strip.

### Layout

```
┌ window (bg: --bg) ────────────────────────────────────────────────┐
│ ┌sidebar┐  ┌ topbar (breadcrumb · search · refresh) ────────────┐ │
│ │       │  ├ main (scrolls) ─────────────────────────────────────┤ │
│ │       │  │                                                     │ │
│ │       │  ├ status line (floating panel) ───────────────────────┤ │
│ └───────┘  └─────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────┘
```

- Row: `display:flex`. The sidebar is `flex:none`; the content column is `flex:1; min-width:0; flex-direction:column`.
- **Sidebar panel:** width 224px expanded, 58px collapsed (`transition: width 180ms ease`). Margin `8px 0 8px 8px`; background `--sb`; `1px solid --bd`; radius 14px; shadow `--fsh`; `overflow:hidden`.
- **Top bar:** height 50px; `margin-top:8px`; padding `0 16px 0 20px`; flex row with a 10px gap. Breadcrumb on the left, then a spacer, the search pill and the refresh icon button.
- **Main:** `flex:1; overflow:auto`; padding `4px 16px 16px 20px`; flex column with an 18px gap. Register is the exception: main uses `overflow:hidden` and the table scrolls itself.
- **Status line panel:** height 34px; margin `0 8px 8px 8px`; same material as the sidebar (`--sb`, `1px --bd`, radius 12px, `--fsh`). Its bottom edge lines up with the sidebar's bottom edge, so both can be glass in Tauri.

### Sidebar contents

- **Header** (50px tall, padding `0 10px`, gap 8px):
  - Hamburger button, 32 × 32, radius 8px, transparent; hover background `--hv`. The icon is 20px with three lines, stroke 2. Toggles collapsed.
  - Wordmark `nigel`: 15px/600, letter-spacing −0.01em, filled with `--grad` via `background-clip:text`. Hidden when collapsed.
- **Nav groups** (padding `0 8px 8px`; 10px gap between groups; 1px gap between items):
  - (no label): Dashboard, Register, Review, Reports
  - **Statements:** Import, Reconcile, Undo
  - **Billing:** Clients, Invoices, Documents
  - **Setup:** Accounts, Categories, Rules
  - Group label: 10.5px/500, letter-spacing .08em, uppercase, `--mu`, padding `4px 10px 3px`. Hidden when collapsed.
- **Nav item:**
  - Button, height 32px, padding `0 9px`, gap 10px, radius 8px, `1px solid transparent` border. Text 13.5px; icon 17px with stroke 2.
  - Hover background `--hv`.
  - **Active:** background `--sel`, weight 500, and a **1px rainbow hairline border** made with `background: linear-gradient(var(--sel),var(--sel)) padding-box, var(--grad) border-box`.
  - Review shows a right-aligned count (`84`, 11px, `--flag`).
  - When collapsed, items centre their icon and the labels are hidden.
- **Footer:** `border-top:1px solid --bs`; padding 8px; contains the Settings nav item.

### Breadcrumb

- **Book button:**
  - Height 30px, padding `0 6px 0 4px`, gap 8px, radius 8px, transparent; hover `--hv`.
  - Monogram tile: 20 × 20, radius 6px, background is the book's colour, text `#2b2b33` at 9px/600.
  - Then the book name (13px, `--tx`) and a 14px chevron-down in `--mu`.
  - Click opens the palette scoped to Books.
- **Separator:** `/` in `--mu`, padding `0 6px`.
- **Screen title:** 13px, `--mu`.
- **Invoice detail:** the trail becomes `Invoices / #1254`. "Invoices" is a button that returns to the list, and `#1254` is in `--tx`.

### Search pill and refresh

- **Search pill:** width 240px, height 32px, radius 999px, `1px --bd`, background `--sf`, text `--mu` at 12.5px.
  - Contents: a 15px magnifier, the word "Search", and a `⌘K` keycap (11px, `1px --bd`, radius 999px, padding `0 7px`, background `--bg`).
  - Hover border `--br`. Click opens the unscoped palette.
- **Refresh:** 32 × 32 round icon button with a 17px icon in `--mu`; hover `--hv`.

### Window title

`{Screen} · {Book name}` (invoice detail: `#1254 · Invoices · Acme Consulting LLC`). Set the Tauri window title to this.

---

## 2. Book switcher / ⌘K palette

Reference: `prototypes/NigelPalette.dc.html`.

### Opening and closing

- ⌘K / Ctrl+K anywhere toggles it, unscoped.
- Clicking the breadcrumb book, the status-line book, or "Switch books…" in Settings opens it scoped to Books.
- Esc or a click on the backdrop closes it.

### Backdrop and panel

- **Backdrop:** `--bk`, covering the whole window.
- **Panel:** absolutely positioned at `top:76px`, horizontally centred; width 620px (max `100% − 48px`); background `--sf`; `1px --bd`; radius 12px; shadow `--sh`.

### Input row

- Height 54px; padding `0 14px 0 16px`.
- Contents: an 18px magnifier in `--mu`, then the input (15px, borderless).
- **Placeholder:** "Search books and screens…" when unscoped; "Switch to another set of books…" when scoped.
- **Scope chip** (Books scope only): `Books ×`, height 24px, radius 6px, background `--sa`. Clicking it widens the search to everything.
- Right side: an `esc` keycap.

### Results

Results area: max-height 430px, scrolls, padding 6px, `border-top:1px --bd`.

- **Section header:** 11px/500, uppercase, letter-spacing .08em, `--mu`. The sections are "Books" and "Go to".
- **Book row:**
  - 3-column grid `32px 1fr auto`; gap 12px; padding `8px 10px`; radius 8px.
  - Monogram: 32 × 32, radius 8px, background is the book's colour, text 11.5px/600 `#2b2b33`.
  - Name: 14px/500. The current book has an `OPEN` badge (10.5px uppercase, `1px --br`, `--br` text, radius 4px).
  - Second line: `{Business|Personal} · {folder path}`, 12px, `--mu`, ellipsised.
  - Right column: a 14px lock icon if the book is encrypted, then the last-opened text (`Today`, `Yesterday`, `Sep 28`; the current book shows `Open now`), 12px, `--mu`, min-width 72px, right-aligned.
- **Action rows**, after the books: `New books…` (plus icon) and `Open books from a folder…` (folder icon), 13.5px.
- **Screen rows** (unscoped only): a 16px icon, the label, and a right-aligned `screen` tag (11.5px, `--mu`). All 14 screens are listed, including "Undo last import".
- **Highlight:** background `--hl` on the keyboard or hover row.
- **Empty state:** `Nothing matches "{q}".`, centred, `--mu`.

### Footer

Height 34px; background `--sa`; 11.5px `--mu`. Shows `↑↓ move  ↵ open  esc close`, with `nigel` in the gradient on the right.

### Behaviour

- Case-insensitive substring match on the book's name, kind and path, on action labels, and on screen labels.
- ↑/↓ moves the highlight through one flat list (books, then actions, then screens). ↵ activates the row.
- Picking a book switches the open book (the real app reloads onto that `nigel.db`; see `docs/desktop.md`). Picking a screen navigates to it.
- Focus the input when the palette opens.

### Data needed per book

`id`, `name`, `kind` (business / personal), `path`, `encrypted`, `lastOpened`, plus a monogram (two uppercase initials from the name) and a colour. The colour is assigned from the pastel ramp `#ffb3ba #ffc8a2 #ffe0a3 #c9ffcb #bae1ff #c4b7ff #ffb3de` (for example by hashing the id); allow a per-book override later.

This needs a recent-books registry on the host (Tauri) side; it does not exist yet.

---

## 3. Status line

Reference: `prototypes/NigelStatusLine.dc.html`, mounted with `variant="floating"`.

### Bar

Full height of the 34px panel; 11.5px; `--mu`; `white-space:nowrap`. Segments run left to right:

1. **`nigel` block:** background `--pastel` (pastel rainbow), text `#2b2b33` at 600, padding `0 10px`, radius 6px, margin `4px 2px 4px 4px`.
2. **Book button:** a 14 × 14 monogram (radius 4px, 7px/600) plus the name in `--tx`. Hover `--hv`. Click opens the palette scoped to Books.
3. **Folder path:** selectable text.
4. **Lock state:** a 12px lock icon with "encrypted", or an open lock with "not encrypted".
5. *(spacer)*
6. **`▲ 84 to review`** in `--flag`. Click goes to Review.
7. **`⌘K palette`.** Click opens the unscoped palette.
8. **`v1.0`.**

Segments have padding `0 12px` and are separated by `1px solid --bs` (right borders before the spacer, left borders after it).

---

## 4. Dashboard

Reference: Nigel App, screen `dashboard`.

1. **Title row:** `Dashboard` as h1, 28px/500, letter-spacing −.02em, line-height 1.1. On the right, the review pill: height 30px, radius 999px, `1px --flag`, transparent, with a 15px flag icon in `--flag`, **84** (600, `--flag`) and "to review". It links to Review.
2. **Stat strip:** one card (background `--sf`, `1px --bd`, radius 12px) holding a 3-column grid. Cells have padding `18px 22px` and are divided by `1px --bs` left borders.
   - Label: 12px `--mu`.
   - Value: 28px/500, letter-spacing −.02em, tabular numerals.
   - Sub-line: 12px `--mu`.
   - Cells: "Income, year to date" (`--inc`); "Expenses, year to date" (`--exp`, shown with a minus sign); "Net" (coloured by sign, sub-line "Cash basis").
3. **Lower grid:** `2fr 1fr`, gap 14px.
   - **Cash flow card** (padding `18px 22px`):
     - Header: title 15px/600, the range "2025 – 26" in `--mu`, and a legend with 8px round dots (`--incf`, `--expf`).
     - Chart: 220px tall, 12 monthly buckets, gap 10px. Each bucket has paired bars at 38% width each, 3px apart, with **fully rounded tops** (`999px 999px 0 0`). A `1px --bd` baseline runs underneath, and month labels (12px `--mu`) sit below.
   - **Balances card:** rows split by `1px --bs`, each with the account name, its type (12px `--mu`) and a right-aligned balance coloured by sign. A **Total** row follows a `1px --bd` rule.

---

## 5. Register

Reference: Nigel App, screen `register`. Rebuilds `screens/register.ts`; keep its existing features (inline edit, keyboard shortcuts, flag toggle).

### Toolbar

One wrapping row, gap 8px, all controls 32px tall with 8px radius:

1. **Account select:** custom chevron; options "All accounts" and each account.
2. **Period stepper:** ‹ label › with the label min-width 132px, 500. The chevrons are disabled (35% opacity) when the period is All.
3. **Segmented control All · Year · Month:** container `--sa` with 3px padding; the active segment has background `--sf`, a small shadow and `--tx` text.
4. **Search field:** `flex:1 1 220px`, with a leading magnifier; placeholder "Description, vendor or category".
5. **Row count:** "123 rows", or "12 of 123 rows" while searching.
6. **"Keys" button:** opens the existing shortcuts help.

Focus ring for all fields: `border-color:--br; box-shadow:0 0 0 3px --ring`.

### Table card

Background `--sf`, `1px --bd`, radius 12px; it fills the remaining height and the body scrolls on its own.

- **Columns:** `40px 104px 1fr 186px 118px 116px 150px`, i.e. flag · Date · Description · Category · Vendor · Amount (right-aligned, 18px right padding) · Account.
- **Header:** 34px tall, background `--sa`, 12px/500 `--mu`, `1px --bd` bottom border.
- **Rows:** 36px tall, 13px, tabular numerals, `1px --bs` bottom border.
  - Hover `--sa`; selected `--hl`.
  - **Left edge stripe**, 2px: `--br` when selected, `--flag` when the row is flagged or uncategorised.
- **Cells:**
  - Flag button: 28 × 28; the flag is filled `--flag` when flagged and an outline `--mu` otherwise. The click toggles the flag and does not select the row.
  - Date: `YYYY-MM-DD` in `--mu`.
  - Category: shows "Uncategorized" in `--mu` when empty.
  - Amount: `--inc` or `--exp`.
- **On open,** the table scrolls to the most recent rows.
- **Footer:** 38px tall, background `--sa`, `1px --bd` top border, 12.5px. The account filter name sits on the left; on the right, "Total" followed by the filtered sum (600, coloured by sign).
- **Empty state:** "No transactions match this search."

---

## 6. Invoices

### List

Reference: Nigel App, screen `invoices`.

1. **Title row:** h1 `Invoices`, a spacer, then two buttons:
   - "Sync now": outline pill, 32px, radius 999px, `1px --bd`, refresh icon. Tooltip: "Pull paid Stripe checkouts and record them".
   - "New invoice": **filled** pill with background `--br` and text `--onbr` at 500, with a plus icon.
2. **Outstanding card** (padding `16px 20px 18px`, gap 12px):
   - Top line: label "Outstanding", the amount (22px/500), and "across 3 open invoices · as of {date}". On the right, an "Aging report →" link in `--br`.
   - **Stacked aging bar:** 10px tall, radius 999px, 3px gaps between segments, track `--bs`. Segments are proportional to the five buckets: Current, 1–30, 31–60, 61–90, Over 90.
     - Colours, light mode: `#5a9473 #c9a13b #d98b4a #cb6b7b #a33b5a`.
     - Colours, dark mode: `#6fcf92 #ffe0a3 #ffc8a2 #ff9fa8 #ffb3de`.
   - Below the bar, a 5-column legend: an 8px swatch, the label (12px `--mu`) and the amount (13.5px; `--mu` when zero).
3. **Status chips:** "Status" label, then All / Draft / Sent / Partial / Overdue / Paid / Void, each with a count in `--mu`.
   - Chips are 28px tall, radius 999px, `1px --bd`.
   - The active chip has border `--br`, background `--hl`, text `--br` at 500.
   - Clicking a chip filters the table.
4. **Table card:**
   - Columns: `84px 1fr 120px 120px 112px 120px 120px 36px`, i.e. Number · Client · Issued · Due · Status · Total · Balance · chevron.
   - Rows are 44px tall at 13.5px; hover `--hl`; clicking a row opens the detail view.
   - **Status pill:** 22px tall, radius 999px, a 1px border, text and a 6px dot all in the status colour.
     - Draft `--mu` · Sent `--blue` · Partial `--flag` · Overdue `--exp` · Paid `--inc` · Void `--mu`.
     - Void rows strike through the status and the total, and show "—" for the balance.
   - Due shows "On receipt" when there is no date. The Due cell is `--exp` when the invoice is overdue.
   - Balance is weight 500, `--tx` when outstanding and `--mu` otherwise.
   - Empty state: "No invoices with this status."

### Detail

Reference: Nigel App, Invoices → click a row.

1. **Title row:** h1 `Invoice #1254` with a status pill (24px tall). On the right, the actions, as 32px pills:
   - **Send…** (Resend… once sent): filled `--br`.
   - **Record payment…**, **Edit**, **Duplicate**: outline `--bd`.
   - **Delete…** for drafts, **Void…** otherwise: outline `--exp`, text `--exp`.
   - Disabled actions sit at 40% opacity.
   - Enablement:
     - Send / Resend: any status except paid or void.
     - Record payment: sent, partial or overdue.
     - Edit: draft or sent.
     - Duplicate: always.
     - Void: any status except void.
2. **Fact strip:** 4 columns in one card.
   - **Client:** name, plus the email in `--mu`.
   - **Issued.**
   - **Due:** `--exp` plus a "19 days overdue" note when overdue.
   - **Balance:** 24px/500, with "of {total}" underneath.
3. **Line items card:**
   - Grid `1fr 70px 120px 130px`: Description · Qty · Rate · Amount.
   - Header 12px `--mu` with a `1px --bd` rule; rows have `1px --bs` rules; then a Total row at 600.
4. **Two cards, side by side:**
   - **Payments:** date · method · amount (`--inc`), or "No payments recorded."
   - **Published:**
     - Two labelled links in `--br`, "Invoice page" and "Payment link", wrapping anywhere.
     - Drafts show: "Nothing published yet. Sending creates the page and the payment link."

---

## 7. Settings (most changed)

Reference: Nigel App, screen `settings`. Replaces `screens/settings.ts`.

### Structure

- Two-column grid `184px 1fr`, gap 36px.
- **Left column:** a sticky index containing the h1 `Settings` (28px/500), then two groups:
  - **This book:** Letterhead, Security, Location.
  - **This device:** Appearance, Updates.
  - Index items are 30px tall, radius 7px, 13px. Inactive items are `--mu`; the active item is `--tx` on `--sa` with a 3 × 14 gradient tick.
  - Clicking an item smooth-scrolls to its section.
  - The index follows the scroll position (scroll-spy; the last section is highlighted at the bottom of the scroll).
- **Right column:** max-width 720px. Each group starts with a scope header:
  - A 28 × 28 tile, followed by a label (11px uppercase `--mu`) and a title (14px/500).
  - This book: the tile is the book monogram and the title is the book name.
  - This device: a pastel tile with a monitor icon, titled "Applies to every set of books".
- **Sections** are separated by `border-top:1px --bd` with padding `20–22px 0 26px`. Section titles are 16px/600, and descriptions are 13px `--mu`.

### Letterhead

- **Description:** "What the invoice page and the PDF say about you. The name also appears in the breadcrumb, on reports and in the window title."
- **Fields:**
  - Two columns: Business name (personal books: "Household name") and Phone.
  - Address: textarea, 3 rows.
  - Payment instructions: textarea, 3 rows. Help text: "Printed on both the invoice page and the PDF, or on neither. Leave it empty if you do not take bank transfers."
  - Inputs are 36px tall, padding `0 11px`, radius 8px, `1px --bd`, background `--field`. Labels are 12.5px/500.
- **Logo dropzone:**
  - `1px dashed --bd`, radius 10px, background `--sa`, padding 12px.
  - Contents: a 120 × 48 preview on white, then "Drop a PNG or JPEG here" with the help text "Up to 128 KiB. It is embedded in the PDF and inlined in the page; Gmail readers see your name in the email body and the logo on the attachment.", and a "Choose…" button.
- **Saving:** there is no per-section Save button. While any letterhead field differs from the saved value, a **sticky save bar** appears at the bottom of main:
  - Pill shape (radius 999px), background `--sf`, `--sh` shadow, aligned to the content column.
  - Contents: a `--flag` dot, "Unsaved changes to the letterhead", **Discard** (outline) and **Save** (filled `--br`).
  - After saving, the section title shows "✓ Saved" in `--inc` for about 2.2s.
  - A saved name updates the breadcrumb, the status line and the window title.

### Security

- **Status card** (padding `14px 16px`, radius 12px, background `--sf`):
  - A 36 × 36 icon tile, radius 10px.
  - **Encrypted:** tile background `--hl` with a `--br` lock; title "Encrypted"; description "This database is encrypted with a password. Lose the password and the books cannot be opened."
  - **Not encrypted:** the card border is `--flag`, and the tile shows an open lock in `--flag`; title "Not encrypted"; description "Anyone with access to the file can read it. Set a password and store it somewhere safe."
- **When encrypted**, one card with two rows:
  - **Change password:** the row reads "Change password" with the sub-line "You will need the current one." and a **Change…** button. It expands inline into a 3-column form: Current password · New password · Type it again, with a filled `--br` "Change password" button. The button label toggles to **Cancel** while open.
  - **Remove password:** the sub-line is "Decrypts the database file." The **Remove…** button is outlined in `--exp`. It expands into a Current password field, a filled `--exp` "Remove password" button, and the warning "The database will be decrypted and readable by anyone with access to the file." in `--exp`.
  - Only one panel is open at a time.
- **When not encrypted:** one card with New password · Type it again and a filled "Encrypt database" button.
- Keep the existing validation and error handling from `settings.ts`.

### Location

- **Description:** "Where this set of books lives. Each set is a folder with a nigel.db inside; switching reloads Nigel onto the other database."
- The path appears in a `code` box (background `--sa`, `1px --bs`, radius 8px, selectable).
- Buttons: **Reveal in Finder** (Tauri: open the folder; hide this in the browser) and **Switch books…** (opens the palette scoped to Books).
- This replaces the old data-directory field.

### Appearance

- Three preview cards in a 3-column grid: **System · Light · Dark**.
- **Card:**
  - Padding 8px, radius 12px.
  - Holds a 78px mini-window preview: a sidebar block, lines and a card, drawn in that theme's colours. The System card is split diagonally between light and dark.
  - Below the preview, a radio dot and the label.
- **Selected card:** a 1.5px rainbow border (the same padding-box/border-box gradient as the active nav item) and a filled radio dot in `--br`.
- Applies immediately. Stored per device (`localStorage` or Tauri store), not in the book.

### Updates

- A card with "Check for updates automatically", the sub-line "Checks GitHub for a newer version once a day when Nigel starts." and a toggle switch.
- **Switch:** 38 × 22, radius 999px, track `--br` when on and `--bd` when off; 16px white knob; 150ms transition.
- Below the card: "You're on v1.0, the latest version." (12px `--mu`).

---

## 8. Setup (onboarding)

Reference: `prototypes/NigelSetup.dc.html` (prop `step`: arrival · profile · identity · first).

### Keep

- `wc-particle-field` and the large ASCII `wc-wordmark` (13px, line-height 1.18, 600).
- The wordmark gradient runs at 160°:
  - Dark: `#ffc8a2 → #ffb3ba → #ffb3de → #c4b7ff → #bae1ff → #c9ffcb`.
  - Light: `#9b5524 → #c22e3b → #ba2c7c → #6951d6 → #266ba1 → #1c781f`.
- A soft radial glow behind it: `radial-gradient(ellipse 55% 45% at 50% 32%, …)` using `rgb(196 183 255 / 10%)` in dark and `/ 35%` in light.

### Changes

- **Layout:** content centred, max-width 640px, 18px gap.
- **Headings:** 24–26px/500, letter-spacing −.02em.
- **Buttons:** 38px pills. The primary is filled `--br` with text `--onbr`; the secondary is an outline in `--bd`.
- **Profile step:** the two choices become **side-by-side cards**.
  - Card: padding 18px, radius 14px, `1px --bd`, background `--sf`.
  - Hover: border `--br` plus a 3px `--ring`.
  - Content: a 15px/600 title and a 13px `--mu` description.
  - Clicking a card picks it and advances.
- **First-books step:** two cards, "Start from scratch" (button "Start fresh") and "Load books I already have" (path input plus a "Load them" button).
- **Step indicator** at the bottom: 6px-tall pills. The current step is 22px wide; the others are 6px wide. Completed and current pills are `--br`; future pills are `--bd`. Width transitions over 200ms.
- **Copy** is unchanged from `setup.ts`, except "Tell me about yourself." on the identity step.

---

## 9. Mobile drawer (narrow viewport)

Reference: `prototypes/NigelMobile.dc.html` (390 × 844; prop `drawer`). The prototype includes a fake status bar; ignore it.

### Top bar

- Height 52px, `1px --bs` bottom border, padding `0 12px`.
- Contents: the hamburger (44 × 44), a compact breadcrumb button (monogram, short book name in `--mu`, `/`, screen name; opens the palette scoped to Books), a spacer, and a 44 × 44 search icon that opens the palette.
- **All touch targets are at least 44px.**

### Drawer

- The same floating-panel material as the sidebar.
- Absolutely positioned: top below the status bar, left 8px, bottom 8px; width 300px; radius 18px; shadow `--sh`.
- Backdrop `--bk`, closed by tapping it.
- **Contents:**
  - Header: a close (×) button and the `nigel` wordmark.
  - The same nav groups, with Settings moved into Setup.
  - Items are 44px tall at 14px.
  - The active item uses the rainbow hairline treatment.
- **Book card** at the foot (replaces the status line on mobile):
  - A 32px monogram, the book name (13.5px/500) and `Encrypted · {path}` (11.5px `--mu`).
  - An up/down chevron.
  - Tap opens the palette scoped to Books.
- Selecting any item closes the drawer.

### Content

- Stats become a single card with stacked rows (label left, 20px/500 value right).
- The chart is 150px tall.
- Cards have 14px radius.

---

## Interactions summary

- **Sidebar collapse:** width 224 ↔ 58 over 180ms ease; labels and group labels hide.
- **⌘K / Ctrl+K:** toggles the palette from anywhere. Esc closes it. ↑↓↵ navigate.
- **Book switch:** updates the breadcrumb, status line, Settings (letterhead, security state, location) and the window title. In the real app, reload onto the selected database.
- **Screen change:** resets main's scroll to the top. Register scrolls its table to the bottom.
- **Focus rings:** inputs use `border-color --br` plus `box-shadow 0 0 0 3px --ring`. Buttons should use `:focus-visible { outline:2px solid var(--br); outline-offset:2px }`.
- **Tauri:**
  - Standard title bar, so the hamburger sits at the sidebar's top-left below it.
  - The sidebar panel and status-line panel are the two surfaces that become native Liquid Glass; give them transparent backgrounds when running under Tauri on macOS.
  - Everything else stays opaque.

## State (suggested)

- **App:**
  - `screen`, `sidebarCollapsed` (persist), `palette: null | 'books' | 'all'`, `currentBookId`.
  - `books[]`: the recent-books registry from the host.
  - `appearance: 'system' | 'light' | 'dark'` (per device).
- **Register:** `account`, `period`, `query`, `selectedId`, flags (server).
- **Invoices:** `statusFilter`, `openInvoiceNumber` (route `#/invoices/1254`).
- **Settings:** `letterheadDraft` vs saved (drives the save bar), `securityPanel: null | 'change' | 'remove'`, `updatesEnabled`, `activeSection` (scroll-spy).

---

## Design tokens

These extend `@nigel/theme` (`tokens/color.ts`, `gradient.ts`). The names are the prototype's shorthand; map them onto the existing `--color-*` roles.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f3f2f7` | `#17171d` | window ground |
| `--sf` | `#ffffff` | `#1f1f28` | cards, palette, fields |
| `--sa` | `#f6f4fb` | `#25252f` | table headers, footers, subtle fills |
| `--sb` | `#fbfafe` | `#22222b` | sidebar and status panels (browser) |
| `--hl` | `#f1edff` | `#2a2740` | highlight, selected row, active chip |
| `--hv` | `#efedf5` | `#2a2a35` | hover |
| `--sel` | `#f1edff` | `#2a2740` | active nav fill |
| `--bd` | `#dedaeb` | `#2e2e3c` | borders |
| `--bs` | `#ebe8f3` | `#26262f` | row and inner dividers |
| `--tx` | `#383843` | `#ece9f5` | text |
| `--mu` | `#63636f` | `#a5a2b5` | muted text |
| `--br` | `#5a3fd6` | `#c4b7ff` | brand: primary buttons, links, focus |
| `--onbr` | `#ffffff` | `#17171d` | text on `--br` |
| `--ring` | `rgb(90 63 214 / 22%)` | `rgb(196 183 255 / 25%)` | focus ring |
| `--field` | `#ffffff` | `#1b1b22` | input background |
| `--inc` | `#17683a` | `#7fe0a0` | income text |
| `--exp` | `#b3283f` | `#ff9fa8` | expense and danger text |
| `--flag` | `#855508` | `#ffe0a3` | review and flag |
| `--blue` | `#266ba1` | `#bae1ff` | Sent status |
| `--incf` | `#5a9473` | `#6fcf92` | income bars |
| `--expf` | `#cb6b7b` | `#f0919c` | expense bars |
| `--grad` | ink rainbow `90deg #c22e3b,#9b5524,#84621f,#1c781f,#266ba1,#6951d6,#ba2c7c` | pastel rainbow `90deg #ffb3ba,#ffc8a2,#ffe0a3,#c9ffcb,#bae1ff,#c4b7ff,#ffb3de` | wordmark, active hairlines |
| `--pastel` | pastel rainbow | pastel rainbow | status-line `nigel` block |
| `--fsh` | `0 1px 2px rgb(43 43 51/6%), 0 8px 24px rgb(43 43 51/6%)` | `0 8px 24px rgb(0 0 0/35%)` | floating panels |
| `--sh` | `0 16px 40px rgb(43 43 51/18%), 0 2px 6px rgb(43 43 51/6%)` | `0 20px 50px rgb(0 0 0/55%), 0 0 0 1px rgb(255 255 255/4%)` | palette, save bar |
| `--bk` | `rgb(40 36 60/18%)` | `rgb(0 0 0/45%)` | modal backdrop |

**Monogram text** is always `#2b2b33` on the pastel book colour.

**Typography:**
- Family: IBM Plex Mono (400 / 500 / 600) everywhere, using the existing `font-faces.ts`. Base 14px, line-height 1.5.
- Sizes:
  - 28px/500, −.02em: page h1.
  - 26 / 24px/500: Setup headings.
  - 22–24px/500: secondary figures.
  - 16px/600: Settings section titles.
  - 15px/600: card titles.
  - 13.5px: nav and body.
  - 13px: controls.
  - 12–12.5px: meta.
  - 11.5px: status line and keycaps.
  - 10.5–11px/500, uppercase, .08em: group labels.
- Tabular numerals on every figure.

**Radius:**
- 4px: badges.
- 6px: chips, small tiles.
- 7–8px: nav items, inputs, icon buttons.
- 10px: dropzone, icon tiles.
- 12px: cards, palette, status panel.
- 14px: sidebar, mobile cards, setup cards.
- 18px: mobile drawer.
- 999px: pills, buttons, switches.

**Spacing:** 4 / 6 / 8 / 10 / 12 / 14 / 16 / 18 / 20 / 22 / 24px, as used above. The window inset for floating panels is 8px.

## Assets

- **Icons:** 24-unit stroke icons (stroke 2, round caps and joins) matching `web/packages/ui/src/icons/icons.ts`. Use the existing set; the prototype paths are stand-ins. New icons needed: lock, lock-open, search, chevron-down, chevron-up-down, flag, plus, folder, monitor, keyboard, x.
- **Wordmark and particles:** the existing `wc-wordmark` and `wc-particle-field`.
- **App icon:** out of scope; still in design.

## Screenshots

`screenshots/` holds static captures of each screen in light and dark (`light-NN-*.png`, `dark-NN-*.png`): Dashboard, Register, Invoices, Invoice detail, Settings (top of page), the switcher scoped to Books, the unscoped palette, and the four Setup steps. `mobile-light-and-dark.png` shows the top bar and the open drawer in both modes. The grey margin at the right edge of each image is capture padding, not part of the design. The lower Settings sections (Security, Location, Appearance, Updates) aren't captured; scroll the Settings prototype to see them.

## Files

- `prototypes/Nigel App.dc.html`: shell, Dashboard, Register, Invoices (list and detail), Settings. Props: `mode` (light|dark), `screen`, `initialPalette`.
- `prototypes/NigelPalette.dc.html`: the ⌘K palette and book switcher.
- `prototypes/NigelStatusLine.dc.html`: the status line.
- `prototypes/NigelSetup.dc.html`: onboarding.
- `prototypes/NigelMobile.dc.html`: mobile top bar and drawer.
- `prototypes/support.js`: the prototype runtime only.
