/* Connects the panel to Jauge (Tauri): the same functions as the Electron version, vue.js has not changed. */
'use strict';
(function () {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  /* In the app, the macOS glass behind the panel; in the self-test, a solid background for the capture. */
  /* On Linux the window is opaque (no glass to show through), so it keeps the solid background too. */
  if (!window.JAUGE_ESSAI && !window.JAUGE_OPAQUE) document.documentElement.classList.add('vibrance');
  window.jauge = {
    charger: () => invoke('etat'),
    surEtat: rappel => listen('etat', e => rappel(e.payload)),
    actualiser: () => invoke('actualiser'),
    connecter: () => invoke('connecter'),
    hauteur: h => invoke('hauteur', { h }),
  };
})();
