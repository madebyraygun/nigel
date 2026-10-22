import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describeButtonOwnership } from '../../../../packages/ui/preview/button-guard.js';

/**
 * The same guard `packages/ui` carries, for the screens: a screen renders
 * `wa-button` and leaves its shape and fill to `controlsCss`.
 */
const here = dirname(fileURLToPath(import.meta.url));

describeButtonOwnership(resolve(here, '../'));
