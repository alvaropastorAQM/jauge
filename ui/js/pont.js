/* Relie le panneau à Jauge (Tauri) : les mêmes fonctions que la version Electron, vue.js n'a pas changé. */
'use strict';
(function () {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  /* Dans l'app, le verre de macOS derrière le panneau ; en auto-test, un fond plein pour la capture. */
  if (!window.JAUGE_ESSAI) document.documentElement.classList.add('vibrance');
  window.jauge = {
    charger: () => invoke('etat'),
    surEtat: rappel => listen('etat', e => rappel(e.payload)),
    actualiser: () => invoke('actualiser'),
    connecter: () => invoke('connecter'),
    hauteur: h => invoke('hauteur', { h }),
  };
})();
