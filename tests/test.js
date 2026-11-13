/* Rythme des limites, tel que le panneau l'affiche (ui/js/rythme.js). node tests/test.js
   L'équivalent API, la lecture des limites et la barre des menus sont testés côté Rust : cargo test dans src-tauri. */
'use strict';
const assert = require('assert/strict');
const R = require('../ui/js/rythme.js');

const proche = (a, b) => assert.ok(Math.abs(a - b) < 1e-9, a + ' ≠ ' + b);
const H = R.HEURE, t = Date.parse('2026-11-08T12:00:00.000Z');

/* 50 % utilisés à mi-fenêtre, c'est dans le rythme ; 86 % à 60 %, c'est très en avance. */
const session = p => ({ groupe: 'session', pourcent: p, reinit: t + 2.5 * H });
assert.equal(R.rythme(session(50), t).etat, 'rythme');
assert.equal(R.libelleRythme(R.rythme(session(50), t)), 'dans le rythme');
const vite = { groupe: 'session', pourcent: 86, reinit: t + 2 * H };
proche(R.rythme(vite, t).ecoule, 0.6);
assert.equal(R.libelleRythme(R.rythme(vite, t)), '26 pts d’avance');
/* Comme Fil : vert en marge, neutre au rythme, jaune en avance, rouge à 15 points d'avance. Mêmes cas que rythme.rs. */
assert.equal(R.ton(vite, t), 'mauvais');
assert.equal(R.ton(session(57), t), 'attention');
assert.equal(R.ton(session(52), t), 'normal');
assert.equal(R.ton(session(48), t), 'marge');
const semaine = { groupe: 'weekly', pourcent: 38, reinit: t + 3.5 * R.JOUR, gravite: 'normal' };
assert.equal(R.libelleRythme(R.rythme(semaine, t)), '12 pts de marge');
assert.equal(R.ton(semaine, t), 'marge');
assert.equal(R.ton({ groupe: 'weekly', pourcent: 95, reinit: null }, t), 'normal');
assert.equal(R.rythme({ groupe: 'weekly', pourcent: 10, reinit: null }, t), null);

assert.equal(R.plusProche([semaine, vite, { groupe: 'weekly', pourcent: null }]), vite);
assert.equal(R.plusProche([]), null);
assert.equal(R.libelle({ groupe: 'weekly', modele: 'Fable' }), 'Fable');
assert.equal(R.libelle({ groupe: 'weekly', type: 'weekly_all' }), 'Semaine');
assert.equal(R.libelle({ groupe: 'weekly', type: 'weekly_oauth_apps' }), 'Semaine · oauth apps');
assert.equal(R.dans(35 * R.MINUTE), 'dans 35 min');
assert.equal(R.dans(2 * H + 5 * R.MINUTE), 'dans 2 h 05');
assert.equal(R.dans(2 * H), 'dans 2 h');
assert.equal(R.dans(30 * H), 'dans 30 h');
assert.equal(R.dans(4.2 * R.JOUR), 'dans 4 j');
assert.equal(R.dans(-1), 'maintenant');

/* Passée la remise à zéro, sans attendre le relevé : la session repart de 0 sans heure, la semaine pour 7 jours de plus. */
const fin = t + 3 * H;
const limites = [{ groupe: 'session', pourcent: 40, reinit: fin - H }, { groupe: 'weekly', pourcent: 74, reinit: fin }, { groupe: 'weekly', pourcent: 10, reinit: fin + 1 }];
assert.deepEqual(R.aJour(limites, fin + R.MINUTE).map(l => [l.pourcent, l.reinit]), [[0, null], [0, fin + 7 * R.JOUR], [0, fin + 1 + 7 * R.JOUR]]);
assert.equal(R.aJour(limites, fin)[1].reinit, fin + 7 * R.JOUR);
assert.equal(R.aJour(limites, fin - 1)[1], limites[1]);

console.log('Tests du panneau : tout est bon.');
