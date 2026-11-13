/* Limites du forfait : rythme, ton, libellés et durées. Partagé par le panneau et le processus principal (barre des menus, tests). */
(function (sortie) {
  'use strict';

  const MINUTE = 60000, HEURE = 60 * MINUTE, JOUR = 24 * HEURE;
  /* Mêmes seuils que Fil : sous 5 points d'écart on est dans le rythme, à 15 points vraiment en avance. */
  const TOLERANCE = 0.05, TRES_EN_AVANCE = 0.15;

  const dureeDe = l => (l.groupe === 'session' ? 5 * HEURE : 7 * JOUR);

  function libelle(l) {
    if (l.groupe === 'session') return 'Session';
    if (l.modele) return l.modele;
    if (l.surface) return l.surface;
    /* Une sorte de limite que Jauge ne connaît pas encore garde un nom à elle, pour ne pas faire deux cartes « Semaine ». */
    if (l.type && !['weekly', 'weekly_all'].includes(l.type)) return 'Semaine · ' + l.type.replace(/^weekly_?/, '').replace(/_/g, ' ');
    return 'Semaine';
  }

  /* Part de la fenêtre écoulée, et écart entre l'utilisation et ce temps écoulé (positif : on consomme plus vite que le temps ne passe). */
  function rythme(l, maintenant) {
    if (l.pourcent == null || !l.reinit) return null;
    const ecoule = Math.min(1, Math.max(0, 1 - (l.reinit - maintenant) / dureeDe(l)));
    const ecart = l.pourcent / 100 - ecoule;
    return { ecoule, ecart, etat: ecart > TOLERANCE ? 'avance' : ecart < -TOLERANCE ? 'marge' : 'rythme' };
  }

  /* Comme Fil : vert quand il reste de la marge sur le temps, neutre au rythme, jaune en avance, rouge vraiment en avance. */
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
    if (r.etat === 'avance') return points + ' pts d’avance';
    if (r.etat === 'marge') return points + ' pts de marge';
    return 'dans le rythme';
  }

  /* Passée sa remise à zéro, une limite repart de 0 sans attendre le prochain relevé : la semaine pour une nouvelle fenêtre de 7 jours,
     la session au prochain message (pas d'heure tant qu'il n'est pas envoyé). */
  const aJour = (limites, t) => limites.map(l => (!l.reinit || l.reinit > t ? l : {
    ...l, pourcent: 0, reinit: l.groupe === 'session' ? null : l.reinit + (Math.floor((t - l.reinit) / dureeDe(l)) + 1) * dureeDe(l),
  }));

  const plusProche = limites => limites.filter(l => l.pourcent != null).reduce((a, l) => (!a || l.pourcent > a.pourcent ? l : a), null);

  function dans(ms) {
    if (ms <= 0) return 'maintenant';
    const min = Math.ceil(ms / MINUTE);
    if (min < 60) return 'dans ' + min + ' min';
    const h = Math.floor(min / 60), m = min % 60;
    if (h < 10) return 'dans ' + h + ' h' + (m ? ' ' + String(m).padStart(2, '0') : '');
    if (h < 48) return 'dans ' + h + ' h';
    return 'dans ' + Math.round(h / 24) + ' j';
  }

  /* « 17:40 » aujourd'hui, « demain 06:59 », sinon « lun. 06:59 ». */
  function quand(t, maintenant) {
    const d = new Date(t), auj = new Date(maintenant);
    const heure = d.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
    const jours = Math.round((new Date(d).setHours(0, 0, 0, 0) - new Date(auj).setHours(0, 0, 0, 0)) / JOUR);
    if (jours === 0) return heure;
    if (jours === 1) return 'demain ' + heure;
    return d.toLocaleDateString('fr-FR', { weekday: 'short' }) + ' ' + heure;
  }

  Object.assign(sortie, { MINUTE, HEURE, JOUR, dureeDe, libelle, rythme, ton, libelleRythme, aJour, plusProche, dans, quand });
})(typeof module !== 'undefined' ? module.exports : (window.Rythme = {}));
