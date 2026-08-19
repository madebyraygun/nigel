import type { CategoryFormValue } from '@nigel/ui';
import type { CategoryPatch, CategoryRow, NewCategoryRequest } from '../api/types.js';

/** Empty means "no value", which on the wire is null rather than "". */
function orNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === '' ? null : trimmed;
}

export function toCategoryForm(row: CategoryRow): CategoryFormValue {
  return {
    name: row.name,
    categoryType: row.categoryType,
    class: row.class,
    taxLine: row.taxLine ?? '',
    formLine: row.formLine ?? '',
  };
}

/**
 * The create body, with `class` present only when the operator chose one.
 *
 * `chosenClass` is not a formality: the control has to show something, and
 * what it shows is a default rather than an answer. Sending it anyway files an
 * income category as an expense — the route derives the right class from the
 * type when the field is absent, and absent is what an untouched control means.
 */
export function newCategoryRequest(
  value: CategoryFormValue,
  chosenClass: boolean,
): NewCategoryRequest {
  return {
    name: value.name.trim(),
    categoryType: value.categoryType,
    ...(chosenClass ? { class: value.class } : {}),
    taxLine: orNull(value.taxLine),
    formLine: orNull(value.formLine),
  };
}

/**
 * The smallest legal PATCH: only what actually changed.
 *
 * The route reads absent as "keep" and `null` as "clear", so an unchanged field
 * must be left out rather than sent back at its current value — two screens
 * editing different fields of one row cannot then blank each other's work.
 */
export function categoryPatch(
  current: CategoryRow,
  next: CategoryFormValue,
): CategoryPatch {
  const patch: CategoryPatch = {};

  const name = next.name.trim();
  if (name !== current.name) patch.name = name;
  if (next.categoryType !== current.categoryType) patch.categoryType = next.categoryType;
  if (next.class !== current.class) patch.class = next.class;

  const taxLine = orNull(next.taxLine);
  if (taxLine !== current.taxLine) patch.taxLine = taxLine;

  const formLine = orNull(next.formLine);
  if (formLine !== current.formLine) patch.formLine = formLine;

  return patch;
}

/**
 * True when there is nothing to send.
 *
 * A body with no recognized field is a 400 by design — "an empty edit is more
 * likely a bug than an intention" — so a save with nothing changed must close
 * the dialog rather than ask.
 */
export function isEmptyPatch(patch: CategoryPatch): boolean {
  return Object.keys(patch).length === 0;
}
