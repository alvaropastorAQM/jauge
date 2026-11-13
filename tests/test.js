/* Pace of the limits, as the panel shows it (ui/js/rythme.js). node tests/test.js
   The API equivalent, the reading of the limits and the menu bar are tested on the Rust side: cargo test in src-tauri. */
'use strict';
const assert = require('assert/strict');
const R = require('../ui/js/rythme.js');

const proche = (a, b) => assert.ok(Math.abs(a - b) < 1e-9, a + ' ≠ ' + b);
const H = R.HEURE, t = Date.parse('2026-11-08T12:00:00.000Z');

/* 50% used halfway through the window is on pace; 86% at 60% is far ahead. */
const session = p => ({ groupe: 'session', pourcent: p, reinit: t + 2.5 * H });
assert.equal(R.rythme(session(50), t).etat, 'rythme');
assert.equal(R.libelleRythme(R.rythme(session(50), t)), 'on pace');
const vite = { groupe: 'session', pourcent: 86, reinit: t + 2 * H };
proche(R.rythme(vite, t).ecoule, 0.6);
assert.equal(R.libelleRythme(R.rythme(vite, t)), '26 pts ahead');
/* Like Fil: green with room to spare, neutral on pace, yellow ahead, red at 15 points ahead. Same cases as rythme.rs. */
assert.equal(R.ton(vite, t), 'mauvais');
assert.equal(R.ton(session(57), t), 'attention');
assert.equal(R.ton(session(52), t), 'normal');
assert.equal(R.ton(session(48), t), 'marge');
const semaine = { groupe: 'weekly', pourcent: 38, reinit: t + 3.5 * R.JOUR, gravite: 'normal' };
assert.equal(R.libelleRythme(R.rythme(semaine, t)), '12 pts to spare');
assert.equal(R.ton(semaine, t), 'marge');
assert.equal(R.ton({ groupe: 'weekly', pourcent: 95, reinit: null }, t), 'normal');
assert.equal(R.rythme({ groupe: 'weekly', pourcent: 10, reinit: null }, t), null);

assert.equal(R.plusProche([semaine, vite, { groupe: 'weekly', pourcent: null }]), vite);
assert.equal(R.plusProche([]), null);
assert.equal(R.libelle({ groupe: 'weekly', modele: 'Fable' }), 'Fable');
assert.equal(R.libelle({ groupe: 'weekly', type: 'weekly_all' }), 'Weekly');
assert.equal(R.libelle({ groupe: 'weekly', type: 'weekly_oauth_apps' }), 'Weekly · oauth apps');
assert.equal(R.dans(35 * R.MINUTE), 'in 35 min');
assert.equal(R.dans(2 * H + 5 * R.MINUTE), 'in 2h 05m');
assert.equal(R.dans(2 * H), 'in 2h');
assert.equal(R.dans(30 * H), 'in 30h');
assert.equal(R.dans(4.2 * R.JOUR), 'in 4d');
assert.equal(R.dans(-1), 'now');

/* Once the reset has passed, without waiting for the reading: the session starts again from 0 with no time, the week for 7 more days. */
const fin = t + 3 * H;
const limites = [{ groupe: 'session', pourcent: 40, reinit: fin - H }, { groupe: 'weekly', pourcent: 74, reinit: fin }, { groupe: 'weekly', pourcent: 10, reinit: fin + 1 }];
assert.deepEqual(R.aJour(limites, fin + R.MINUTE).map(l => [l.pourcent, l.reinit]), [[0, null], [0, fin + 7 * R.JOUR], [0, fin + 1 + 7 * R.JOUR]]);
assert.equal(R.aJour(limites, fin)[1].reinit, fin + 7 * R.JOUR);
assert.equal(R.aJour(limites, fin - 1)[1], limites[1]);

console.log('Panel tests: all good.');
