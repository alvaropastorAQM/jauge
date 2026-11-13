/* Plan limits: pace, tone, labels and durations. Shared by the panel and the main process (menu bar, tests). */
(function (sortie) {
  'use strict';

  const MINUTE = 60000, HEURE = 60 * MINUTE, JOUR = 24 * HEURE;
  /* Same thresholds as Fil: within 5 points we are on pace, at 15 points we are really ahead. */
  const TOLERANCE = 0.05, TRES_EN_AVANCE = 0.15;

  const dureeDe = l => (l.groupe === 'session' ? 5 * HEURE : 7 * JOUR);

  function libelle(l) {
    if (l.groupe === 'session') return 'Session';
    if (l.modele) return l.modele;
    if (l.surface) return l.surface;
    /* A kind of limit that Jauge does not know yet gets a name of its own, so there are never two "Weekly" cards. */
    if (l.type && !['weekly', 'weekly_all'].includes(l.type)) return 'Weekly · ' + l.type.replace(/^weekly_?/, '').replace(/_/g, ' ');
    return 'Weekly';
  }

  /* Share of the window that has elapsed, and the gap between usage and that elapsed time (positive: usage is going faster than time). */
  function rythme(l, maintenant) {
    if (l.pourcent == null || !l.reinit) return null;
    const ecoule = Math.min(1, Math.max(0, 1 - (l.reinit - maintenant) / dureeDe(l)));
    const ecart = l.pourcent / 100 - ecoule;
    return { ecoule, ecart, etat: ecart > TOLERANCE ? 'avance' : ecart < -TOLERANCE ? 'marge' : 'rythme' };
  }

  /* Like Fil: green when there is room to spare on the clock, neutral on pace, yellow when ahead, red when really ahead. */
  function ton(l, maintenant) {
    const r = rythme(l, maintenant);
    if (!r) return 'normal';
    if (r.ecart >= TRES_EN_AVANCE) return 'mauvais';
    if (r.etat === 'avance') return 'attention';
    return r.ecart < 0 ? 'marge' : 'normal';
  }

  function libelleRythme(r) {
    if (!r) return '';
    const points = Math.round(Math.abs(r.ecart) * 100);
    if (r.etat === 'avance') return points + ' pts ahead';
    if (r.etat === 'marge') return points + ' pts to spare';
    return 'on pace';
  }

  /* Once its reset has passed, a limit starts again from 0 without waiting for the next reading: the week for a new 7-day window,
     the session at the next message (no time until it is sent). */
  const aJour = (limites, t) => limites.map(l => (!l.reinit || l.reinit > t ? l : {
    ...l, pourcent: 0, reinit: l.groupe === 'session' ? null : l.reinit + (Math.floor((t - l.reinit) / dureeDe(l)) + 1) * dureeDe(l),
  }));

  const plusProche = limites => limites.filter(l => l.pourcent != null).reduce((a, l) => (!a || l.pourcent > a.pourcent ? l : a), null);

  function dans(ms) {
    if (ms <= 0) return 'now';
    const min = Math.ceil(ms / MINUTE);
    if (min < 60) return 'in ' + min + ' min';
    const h = Math.floor(min / 60), m = min % 60;
    if (h < 10) return 'in ' + h + 'h' + (m ? ' ' + String(m).padStart(2, '0') + 'm' : '');
    if (h < 48) return 'in ' + h + 'h';
    return 'in ' + Math.round(h / 24) + 'd';
  }

  /* "17:40" today, "tomorrow 06:59", otherwise "Mon 06:59". */
  function quand(t, maintenant) {
    const d = new Date(t), auj = new Date(maintenant);
    const heure = d.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });
    const jours = Math.round((new Date(d).setHours(0, 0, 0, 0) - new Date(auj).setHours(0, 0, 0, 0)) / JOUR);
    if (jours === 0) return heure;
    if (jours === 1) return 'tomorrow ' + heure;
    return d.toLocaleDateString('en-GB', { weekday: 'short' }) + ' ' + heure;
  }

  Object.assign(sortie, { MINUTE, HEURE, JOUR, dureeDe, libelle, rythme, ton, libelleRythme, aJour, plusProche, dans, quand });
})(typeof module !== 'undefined' ? module.exports : (window.Rythme = {}));
