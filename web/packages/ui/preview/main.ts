import '@nigel/theme/css/nigel.css';
import { initColorMode } from '@nigel/theme';
import { loadPreviews } from './manifest.js';
import './app/preview-app.js';

// Half the component states in this library have never been looked at in dark
// mode; the harness is where that gets fixed.
initColorMode();

const previews = loadPreviews();
const app = document.createElement('preview-app');
(app as HTMLElement & { previews: typeof previews }).previews = previews;
document.body.appendChild(app);
