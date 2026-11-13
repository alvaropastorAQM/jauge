/* The panel: receives each reading from the main process and redraws itself. Nothing to reload. */
'use strict';
(function () {
  const $ = id => document.getElementById(id);
  const R = window.Rythme;
  let etat = null;
  const cartes = new Map();

  if (new URLSearchParams(location.search).get('vibrance')) document.documentElement.classList.add('vibrance');

  function el(balise, classe, texte) {
    const n = document.createElement(balise);
    if (classe) n.className = classe;
    if (texte != null) n.textContent = texte;
    return n;
  }

  const HORLOGE = '<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" aria-hidden="true"><circle cx="6" cy="6" r="4.8"/><path d="M6 3.4V6l1.7 1"/></svg>';

  /* The number big, the unit smaller (Graphite design, section 4). */
  function chiffre(texte, unite) {
    const f = document.createDocumentFragment();
    f.append(texte, el('span', 'unite', unite));
    return f;
  }

  function montant(usd, usdParEuro) {
    const v = usdParEuro ? usd / usdParEuro : usd;
    const parts = new Intl.NumberFormat('en-GB', { style: 'currency', currency: usdParEuro ? 'EUR' : 'USD', maximumFractionDigits: v >= 100 ? 0 : 2 }).formatToParts(v);
    const f = document.createDocumentFragment();
    for (const p of parts) f.append(p.type === 'currency' ? el('span', 'unite', p.value) : p.value);
    return f;
  }

  const montantTexte = (usd, usdParEuro) => new Intl.NumberFormat('en-GB', {
    style: 'currency', currency: usdParEuro ? 'EUR' : 'USD', maximumFractionDigits: 0,
  }).format(usdParEuro ? usd / usdParEuro : usd);

  const cle = l => [l.groupe, l.type, l.modele, l.surface].join('|');
  const nomForfait = f => (f ? f.charAt(0).toUpperCase() + f.slice(1) : '');
  const heure = t => new Date(t).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' });

  function creerCarte(l, grande) {
    const c = el('article', 'carte ' + (grande ? 'grande' : 'petite'));
    const haut = el('div', 'carte-haut');
    const nom = el('span', 'carte-nom');
    const pct = el('span', 'carte-pct');
    haut.append(nom, pct);
    const piste = el('div', 'piste');
    piste.setAttribute('role', 'meter');
    piste.setAttribute('aria-valuemin', '0');
    piste.setAttribute('aria-valuemax', '100');
    const rempli = el('div', 'rempli'), voile = el('div', 'voile'), repere = el('div', 'repere');
    piste.append(rempli, voile, repere);
    const bas = el('div', 'carte-bas');
    const reinit = el('span', 'reinit'), rythme = el('span', 'rythme'), dans = document.createTextNode('');
    reinit.innerHTML = HORLOGE;
    reinit.append(dans);
    bas.append(reinit, rythme);
    c.append(haut, piste, bas);
    return { c, nom, pct, piste, rempli, voile, repere, reinit, dans, rythme };
  }

  function majCarte(k, l, t) {
    const r = R.rythme(l, t), p = Math.min(100, Math.max(0, l.pourcent || 0));
    k.nom.textContent = R.libelle(l);
    k.pct.replaceChildren(chiffre(String(Math.round(p)), '%'));
    k.piste.setAttribute('aria-valuenow', String(Math.round(p)));
    k.piste.setAttribute('aria-label', R.libelle(l) + ': ' + Math.round(p) + '% used');
    /* As in Fil: the white part is usage, the tick is elapsed time, and between the two a veil:
       green when there is room to spare, yellow or red when usage has passed the tick. */
    const ecoule = r ? r.ecoule * 100 : p;
    k.rempli.style.width = Math.min(p, ecoule) + '%';
    k.voile.className = 'voile ' + R.ton(l, t);
    k.voile.style.left = Math.min(p, ecoule) + '%';
    k.voile.style.width = Math.abs(p - ecoule) + '%';
    k.repere.hidden = !r;
    k.repere.style.left = ecoule + '%';
    k.repere.title = r ? Math.round(ecoule) + '% of the window elapsed' : '';
    /* Just "in X h" to fit in the card; the exact day and time are in the tooltip.
       A session that has not started has no time: its 5 hours start from the next message. */
    k.dans.data = l.reinit ? R.dans(l.reinit - t) : l.groupe === 'session' ? 'starts with your next message' : 'no reset scheduled';
    k.reinit.title = l.reinit ? 'Resets ' + new Date(l.reinit).toLocaleString('en-GB', { weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' }) : '';
    k.rythme.textContent = R.libelleRythme(r);
    k.rythme.className = 'rythme ' + (r ? r.etat : '');
    k.rythme.title = r ? Math.round(p) + '% used, ' + Math.round(ecoule) + '% of the window elapsed' : '';
  }

  function rendreLimites(limites, t) {
    const zone = $('limites');
    if (!limites.length && etat.deconnecte) {
      cartes.clear();
      const bloc = el('div', 'connexion');
      bloc.append(el('p', 'etat-vide', etat.connexion
        ? 'Finish signing in from your browser: Jauge picks up as soon as it succeeds.'
        : 'Claude Code is no longer signed in to your account on this computer. One sign-in is enough, for Jauge as well as for Fil and Tracker.'));
      const bouton = el('button', 'btn-primaire', etat.connexion ? 'Signing in…' : 'Sign in');
      bouton.type = 'button';
      bouton.disabled = Boolean(etat.connexion);
      bouton.addEventListener('click', () => window.jauge.connecter());
      bloc.append(bouton);
      zone.replaceChildren(bloc);
      return;
    }
    if (!limites.length) {
      cartes.clear();
      const texte = etat.erreur
        ? 'Could not read your limits: ' + etat.erreur + ' Check that Claude Code is signed in to your account on this computer.'
        : 'Reading your limits from your Claude account…';
      zone.replaceChildren(el('p', 'etat-vide' + (etat.erreur ? ' erreur' : ''), texte));
      return;
    }
    /* The session first and large, then the week, the limits per use, and those per model. */
    const rang = l => (l.groupe === 'session' ? 0 : l.modele ? 3 : l.surface ? 2 : 1);
    const ordre = [...limites].sort((a, b) => rang(a) - rang(b));
    /* One key per card, even if two limits looked alike: otherwise one of the two would stay empty. */
    const vues = new Map();
    const cles = ordre.map(l => { const c = cle(l), n = (vues.get(c) || 0) + 1; vues.set(c, n); return n > 1 ? c + '#' + n : c; });
    if (cles.join('\n') !== [...cartes.keys()].join('\n')) {
      cartes.clear();
      zone.replaceChildren();
      ordre.forEach((l, i) => {
        const k = creerCarte(l, i === 0 && l.groupe === 'session');
        cartes.set(cles[i], k);
        zone.append(k.c);
      });
    }
    ordre.forEach((l, i) => majCarte(cartes.get(cles[i]), l, t));
  }

  function rendre() {
    if (!etat) return;
    const t = Date.now();
    const frais = etat.releve && t - etat.releve < 150000 && !etat.erreur;

    $('forfait').hidden = !etat.forfait;
    $('forfait').textContent = nomForfait(etat.forfait);
    $('direct').className = 'direct' + (frais ? ' frais' : '') + (etat.enCours ? ' actif' : '');
    $('direct-texte').textContent = frais ? 'live' : etat.deconnecte ? 'signed out' : etat.releve ? 'updated at ' + heure(etat.releve) : etat.erreur ? 'offline' : 'connecting…';

    const c = etat.cout;
    if (c) {
      $('cout').replaceChildren(montant(c.totalUsd, etat.usdParEuro));
      const debut = c.depuis ? new Date(c.depuis).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: new Date(c.depuis).getFullYear() === new Date(t).getFullYear() ? undefined : 'numeric' }) : null;
      $('cout-contexte').textContent = (debut ? 'since ' + debut : 'no messages read') + '\nincluding ' + montantTexte(c.aujourdhuiUsd, etat.usdParEuro) + ' today';
      $('cout').title = 'What your usage recorded in the Claude Code logs would have cost at API prices'
        + (etat.usdParEuro ? ', converted at the ECB rate (€1 = $' + etat.usdParEuro + ')' : '') + '. ' + c.messages.toLocaleString('en-GB') + ' responses counted.';
    }

    const limites = R.aJour(etat.limites || [], t);
    const p = R.plusProche(limites);
    const proche = $('proche');
    if (p) {
      proche.replaceChildren(chiffre(String(Math.round(p.pourcent)), '%'));
      $('proche-contexte').textContent = R.libelle(p) + (p.reinit ? ' · ' + R.quand(p.reinit, t) + '\n' + 'resets ' + R.dans(p.reinit - t) : '');
    } else {
      proche.textContent = etat.erreur || etat.deconnecte ? '-' : '…';
      $('proche-contexte').textContent = '';
    }
    $('logo-arc').style.strokeDasharray = (p ? Math.min(100, p.pourcent) : 0) + ' 100';

    rendreLimites(limites, t);

    const pied = $('pied');
    pied.classList.toggle('erreur', Boolean(etat.erreur && etat.releve));
    pied.classList.toggle('actif', Boolean(etat.enCours));
    $('pied-texte').textContent = etat.enCours ? 'Refreshing…'
      : etat.erreur && etat.releve ? 'No response from Claude: figures from ' + heure(etat.releve) + '.'
      : etat.releve ? 'Updated at ' + heure(etat.releve) + ' · every minute' : '';
  }

  /* The panel takes the height of its content. */
  new ResizeObserver(() => window.jauge.hauteur($('panneau').getBoundingClientRect().height)).observe($('panneau'));

  $('actualiser').addEventListener('click', () => window.jauge.actualiser());
  window.jauge.surEtat(e => { etat = e; rendre(); });
  window.jauge.charger().then(e => { etat = e; rendre(); });
  /* Countdowns and the elapsed-time tick move forward even without a new reading. */
  setInterval(() => { if (!document.hidden) rendre(); }, 15000);
})();
