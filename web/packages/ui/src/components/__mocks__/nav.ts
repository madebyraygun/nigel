import type { NavItem } from '../wc-nav-sidebar.js';

/** A nav list shaped like the app's real one, for previews and tests. */
export const NAV_ITEMS: NavItem[] = [
  { id: 'dashboard', label: 'Dashboard', icon: 'wc-icon-dashboard' },
  { id: 'register', label: 'Register', icon: 'wc-icon-register' },
  { id: 'review', label: 'Review', icon: 'wc-icon-review', count: 84 },
  { id: 'reports', label: 'Reports', icon: 'wc-icon-report' },
  { id: 'import', label: 'Import', icon: 'wc-icon-import', group: 'Statements' },
  { id: 'reconcile', label: 'Reconcile', icon: 'wc-icon-reconcile', group: 'Statements' },
  { id: 'undo', label: 'Undo', icon: 'wc-icon-undo', group: 'Statements' },
  { id: 'clients', label: 'Clients', icon: 'wc-icon-clients', group: 'Billing' },
  { id: 'invoices', label: 'Invoices', icon: 'wc-icon-invoice', group: 'Billing' },
  { id: 'documents', label: 'Documents', icon: 'wc-icon-edit', group: 'Billing' },
  { id: 'accounts', label: 'Accounts', icon: 'wc-icon-account', group: 'Setup' },
  { id: 'categories', label: 'Categories', icon: 'wc-icon-category', group: 'Setup' },
  { id: 'rules', label: 'Rules', icon: 'wc-icon-rule', group: 'Setup' },
  { id: 'settings', label: 'Settings', icon: 'wc-icon-settings', pinned: true },
];

export const NAV_ITEMS_WITH_DISABLED: NavItem[] = NAV_ITEMS.map((item) =>
  item.id === 'dashboard' ? item : { ...item, disabled: true },
);
