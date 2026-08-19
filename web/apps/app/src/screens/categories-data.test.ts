import { describe, it, expect } from 'vitest';
import {
  categoryPatch,
  isEmptyPatch,
  newCategoryRequest,
  toCategoryForm,
} from './categories-data.js';
import type { CategoryRow } from '../api/types.js';

const row: CategoryRow = {
  id: 12,
  name: 'Software / Subscriptions',
  categoryType: 'expense',
  class: 'expense',
  taxLine: 'Other expenses',
  formLine: '1120S-19',
};

describe('toCategoryForm', () => {
  it('renders nulls as empty fields', () => {
    expect(toCategoryForm({ ...row, taxLine: null, formLine: null })).toEqual({
      name: 'Software / Subscriptions',
      categoryType: 'expense',
      class: 'expense',
      taxLine: '',
      formLine: '',
    });
  });
});

describe('newCategoryRequest', () => {
  it('trims, and sends an empty optional field as null', () => {
    expect(
      newCategoryRequest({
        name: '  Consulting income  ',
        categoryType: 'income',
        class: 'revenue',
        taxLine: '',
        formLine: '   ',
      }),
    ).toEqual({
      name: 'Consulting income',
      categoryType: 'income',
      class: 'revenue',
      taxLine: null,
      formLine: null,
    });
  });
});

describe('categoryPatch', () => {
  it('sends nothing when nothing changed', () => {
    expect(categoryPatch(row, toCategoryForm(row))).toEqual({});
  });

  it('sends only the field that changed', () => {
    expect(categoryPatch(row, { ...toCategoryForm(row), name: 'Software' })).toEqual({
      name: 'Software',
    });
  });

  it('clears a field with an explicit null rather than an empty string', () => {
    // Absent keeps and null clears, so "" would leave the old value in place.
    expect(categoryPatch(row, { ...toCategoryForm(row), taxLine: '' })).toEqual({
      taxLine: null,
    });
  });

  it('treats whitespace as a clear', () => {
    expect(categoryPatch(row, { ...toCategoryForm(row), formLine: '   ' })).toEqual({
      formLine: null,
    });
  });

  it('ignores a change that is only surrounding whitespace', () => {
    expect(
      categoryPatch(row, { ...toCategoryForm(row), name: '  Software / Subscriptions  ' }),
    ).toEqual({});
  });

  it('carries a type change', () => {
    expect(categoryPatch(row, { ...toCategoryForm(row), categoryType: 'income' })).toEqual(
      { categoryType: 'income' },
    );
  });

  it('carries several changes at once', () => {
    expect(
      categoryPatch(row, {
        name: 'Software',
        categoryType: 'income',
        class: 'expense',
        taxLine: 'Gross receipts',
        formLine: '',
      }),
    ).toEqual({
      name: 'Software',
      categoryType: 'income',
      taxLine: 'Gross receipts',
      formLine: null,
    });
  });

  it('fills a field that was null before', () => {
    expect(
      categoryPatch({ ...row, formLine: null }, { ...toCategoryForm(row), formLine: 'K-16d' }),
    ).toEqual({ formLine: 'K-16d' });
  });
});

describe('isEmptyPatch', () => {
  it('recognizes the patch that must never be sent', () => {
    expect(isEmptyPatch({})).toBe(true);
    expect(isEmptyPatch({ taxLine: null })).toBe(false);
  });
});

describe('class round trips through the form', () => {
  it('carries the class into the form and back out on create', () => {
    const row: CategoryRow = {
      id: 9,
      name: 'Member Draw',
      categoryType: 'expense',
      class: 'equity',
      taxLine: null,
      formLine: null,
    };
    expect(toCategoryForm(row).class).toBe('equity');
    expect(newCategoryRequest(toCategoryForm(row)).class).toBe('equity');
  });

  it('patches the class alone and leaves an unchanged class out', () => {
    const current: CategoryRow = {
      id: 9,
      name: 'Member Draw',
      categoryType: 'expense',
      class: 'expense',
      taxLine: null,
      formLine: null,
    };
    const next = { ...toCategoryForm(current), class: 'equity' };
    expect(categoryPatch(current, next)).toEqual({ class: 'equity' });
    expect(categoryPatch(current, toCategoryForm(current))).toEqual({});
  });
});
