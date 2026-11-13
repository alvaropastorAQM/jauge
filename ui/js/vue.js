/* Le panneau : reçoit chaque relevé du processus principal et se redessine seul. Rien à recharger. */
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

  /* Le chiffre en grand, l'unité en plus petit (design Graphite, section 4). */
  function chiffre(texte, unite) {
    const f = document.createDocumentFragment();
    f.append(texte, el('span', 'unite', unite));
    return f;
  }

  function montant(usd, usdParEuro) {
    const v = usdParEuro ? usd / usdParEuro : usd;
    const parts = new Intl.NumberFormat('fr-FR', { style: 'currency', currency: usdParEuro ? 'EUR' : 'USD', maximumFractionDigits: v >= 100 ? 0 : 2 }).formatToParts(v);
    const f = document.createDocumentFragment();
    for (const p of parts) f.append(p.type === 'currency' ? el('span', 'unite', p.value) : p.value);
    return f;
  }

  const montantTexte = (usd, usdParEuro) => new Intl.NumberFormat('fr-FR', {
    style: 'currency', currency: usdParEuro ? 'EUR' : 'USD', maximumFractionDigits: 0,
  }).format(usdParEuro ? usd / usdParEuro : usd);

  const cle = l => [l.groupe, l.type, l.modele, l.surface].join('|');
  const nomForfait = f => (f ? f.charAt(0).toUpperCase() + f.slice(1) : '');
  const heure = t => new Date(t).toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });

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
    k.pct.replaceChildren(chiffre(String(Math.round(p)), ' %'));
    k.piste.setAttribute('aria-valuenow', String(Math.round(p)));
    k.piste.setAttribute('aria-label', R.libelle(l) + ' : ' + Math.round(p) + ' % utilisés');
    /* Comme dans Fil : la partie blanche est l'utilisation, le trait le temps écoulé, et entre les deux un voile
       vert quand il reste de la marge, jaune ou rouge quand on a dépassé le trait. */
    const ecoule = r ? r.ecoule * 100 : p;
    k.rempli.style.width = Math.min(p, ecoule) + '%';
    k.voile.className = 'voile ' + R.ton(l, t);
    k.voile.style.left = Math.min(p, ecoule) + '%';
    k.voile.style.width = Math.abs(p - ecoule) + '%';
    k.repere.hidden = !r;
    k.repere.style.left = ecoule + '%';
    k.repere.title = r ? Math.round(ecoule) + ' % de la fenêtre écoulée' : '';
    /* Juste « dans X h » pour tenir dans la carte ; le jour et l'heure exacts sont dans l'infobulle.
       Une session pas encore commencée n'a pas d'heure : ses 5 h partent du prochain message. */
    k.dans.data = l.reinit ? R.dans(l.reinit - t) : l.groupe === 'session' ? 'commence au prochain message' : 'pas de remise à zéro prévue';
    k.reinit.title = l.reinit ? 'Remise à zéro le ' + new Date(l.reinit).toLocaleString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', hour: '2-digit', minute: '2-digit' }) : '';
    k.rythme.textContent = R.libelleRythme(r);
    k.rythme.className = 'rythme ' + (r ? r.etat : '');
    k.rythme.title = r ? Math.round(p) + ' % utilisés pour ' + Math.round(ecoule) + ' % de la fenêtre écoulée' : '';
  }

  function rendreLimites(limites, t) {
    const zone = $('limites');
    if (!limites.length && etat.deconnecte) {
      cartes.clear();
      const bloc = el('div', 'connexion');
      bloc.append(el('p', 'etat-vide', etat.connexion
        ? 'Termine la connexion dans ton navigateur : Jauge reprend dès qu’elle aboutit.'
        : 'Claude Code n’est plus connecté à ton compte sur ce Mac. Une seule connexion suffit, pour Jauge comme pour Fil et Tracker.'));
      const bouton = el('button', 'btn-primaire', etat.connexion ? 'Connexion en cours…' : 'Se connecter');
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
        ? 'Impossible de lire tes limites : ' + etat.erreur + ' Vérifie que Claude Code est connecté à ton compte sur ce Mac.'
        : 'Lecture de tes limites sur ton compte Claude…';
      zone.replaceChildren(el('p', 'etat-vide' + (etat.erreur ? ' erreur' : ''), texte));
      return;
    }
    /* La session d'abord en grand, puis la semaine, les limites par usage, et celles par modèle. */
    const rang = l => (l.groupe === 'session' ? 0 : l.modele ? 3 : l.surface ? 2 : 1);
    const ordre = [...limites].sort((a, b) => rang(a) - rang(b));
    /* Une clé par carte, même si deux limites se ressemblaient : sinon l'une des deux resterait vide. */
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
    $('direct-texte').textContent = frais ? 'en direct' : etat.deconnecte ? 'non connectée' : etat.releve ? 'relevé à ' + heure(etat.releve) : etat.erreur ? 'hors ligne' : 'connexion…';

    const c = etat.cout;
    if (c) {
      $('cout').replaceChildren(montant(c.totalUsd, etat.usdParEuro));
      const debut = c.depuis ? new Date(c.depuis).toLocaleDateString('fr-FR', { day: 'numeric', month: 'short', year: new Date(c.depuis).getFullYear() === new Date(t).getFullYear() ? undefined : 'numeric' }) : null;
      $('cout-contexte').textContent = (debut ? 'depuis le ' + debut : 'aucun échange lu') + '\ndont ' + montantTexte(c.aujourdhuiUsd, etat.usdParEuro) + ' aujourd’hui';
      $('cout').title = 'Ce que ta consommation inscrite dans les journaux de Claude Code aurait coûté au tarif de l’API'
        + (etat.usdParEuro ? ', converti au taux de la BCE (1 € = ' + etat.usdParEuro + ' $)' : '') + '. ' + c.messages.toLocaleString('fr-FR') + ' réponses comptées.';
    }

    const limites = R.aJour(etat.limites || [], t);
    const p = R.plusProche(limites);
    const proche = $('proche');
    if (p) {
      proche.replaceChildren(chiffre(String(Math.round(p.pourcent)), ' %'));
      $('proche-contexte').textContent = R.libelle(p) + (p.reinit ? ' · ' + R.quand(p.reinit, t) + '\n' + 'remise à zéro ' + R.dans(p.reinit - t) : '');
    } else {
      proche.textContent = etat.erreur || etat.deconnecte ? '–' : '…';
      $('proche-contexte').textContent = '';
    }
    $('logo-arc').style.strokeDasharray = (p ? Math.min(100, p.pourcent) : 0) + ' 100';

    rendreLimites(limites, t);

    const pied = $('pied');
    pied.classList.toggle('erreur', Boolean(etat.erreur && etat.releve));
    pied.classList.toggle('actif', Boolean(etat.enCours));
    $('pied-texte').textContent = etat.enCours ? 'Actualisation…'
      : etat.erreur && etat.releve ? 'Pas de réponse de Claude : chiffres de ' + heure(etat.releve) + '.'
      : etat.releve ? 'Relevé à ' + heure(etat.releve) + ' · chaque minute' : '';
  }

  /* Le panneau prend la hauteur de son contenu. */
  new ResizeObserver(() => window.jauge.hauteur($('panneau').getBoundingClientRect().height)).observe($('panneau'));

  $('actualiser').addEventListener('click', () => window.jauge.actualiser());
  window.jauge.surEtat(e => { etat = e; rendre(); });
  window.jauge.charger().then(e => { etat = e; rendre(); });
  /* Les comptes à rebours et le repère du temps écoulé avancent même sans nouveau relevé. */
  setInterval(() => { if (!document.hidden) rendre(); }, 15000);
})();
