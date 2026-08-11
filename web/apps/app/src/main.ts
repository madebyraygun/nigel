import '@nigel/theme/css/nigel.css';
import { initColorMode } from '@nigel/theme';
import '@nigel/ui';
import './components/nigel-app.js';

// index.html has already done this inline, before first paint. This is the
// same work through the module that owns the contract, so the HTML copy stays
// a pure optimisation rather than the only writer.
initColorMode();
