# Jauge

Tes limites Claude en direct dans la barre des menus : un anneau et le pourcentage de ta session de 5 h. Un clic ouvre le détail, un clic droit le menu (actualiser, ouverture au démarrage, quitter). Pas de fenêtre, pas d'icône dans le Dock.

Construite avec Tauri : le fond en Rust (`src-tauri/`), le panneau en HTML, CSS et JavaScript (`ui/`), affiché par le WebKit de macOS. Environ 90 Mo de mémoire en tout, contre 195 Mo pour l'ancienne version Electron.

## Ce qu'elle montre

- **Session de 5 h, semaine, et limites par modèle** (Fable) : pourcentage, heure de remise à zéro, compte à rebours.
- **Le rythme, comme dans Fil :** la partie blanche de chaque barre est ce que tu as utilisé, le trait le temps écoulé dans la fenêtre. Entre les deux, un voile vert quand il te reste de la marge, jaune quand tu es en avance sur le temps, rouge à 15 points d'avance ou plus.
- **L'équivalent API :** ce qu'aurait coûté ta consommation au tarif de l'API, depuis le premier échange lu dans tes journaux, et la part d'aujourd'hui.

Les chiffres restent neutres : seule la barre porte la couleur. L'anneau de la barre des menus prend la couleur du voile de la session (orange à la place du jaune, plus lisible en haut de l'écran).

## D'où viennent les chiffres

- **Limites :** le binaire de Claude livré avec le SDK Claude Agent (0.3.319, épinglé dans `package.json`), avec la connexion Claude Code du Mac. Jauge lui envoie la même demande que le SDK (`get_usage`, marquée expérimentale) : à revérifier si tu mets le SDK à jour. Relevé chaque minute, et dès que Claude écrit dans ses journaux (au plus toutes les 20 s), au réveil du Mac, à ton retour et à l'ouverture du panneau. Mac verrouillé ou sans toi depuis 5 min : un relevé toutes les 5 min. À l'heure de remise à zéro, la limite repart de 0 sans attendre le relevé. Sans relevé réussi depuis 10 min, l'anneau passe en gris.
- **Équivalent API :** les journaux de `~/.claude/projects`, aux tarifs publics relevés au 06/10/2026 dans `src-tauri/src/cout.rs` (à mettre à jour si Anthropic change ses prix), converti en euros au taux du jour de la BCE. Seuls les journaux qui bougent sont relus ; la position de lecture est gardée dans `~/Library/Application Support/Jauge/equivalent-api.json`, ce qui évite de tout relire à chaque démarrage. Un journal effacé par Claude Code reste compté.

## Installer ou reconstruire

Il faut Node.js 22, Rust et les outils Xcode.

```bash
npm install
```

```bash
npm run install-app
```

La seconde commande lance les tests, construit `Jauge.app`, la vérifie, la place dans `/Applications` et l'ouvre. Au premier lancement, elle s'inscrit pour s'ouvrir avec le Mac (décochable dans le menu du clic droit).

## Si l'icône n'apparaît pas

Au premier lancement, Jauge se place à côté de la batterie, du côté visible de l'encoche (`PLACE_BARRE` dans `src-tauri/src/macos.rs`). Sur un Mac à encoche, la barre n'a pas de place pour tout : c'est alors une autre icône qui passe derrière le chevron « « ». Pour changer la place de Jauge, glisse l'anneau en tenant ⌘ ; macOS la retient.

## Tester

`npm test` vérifie les calculs du panneau (JavaScript) et du fond (Rust). `JAUGE_ESSAI=<dossier> JAUGE_DEMO=1 src-tauri/target/release/jauge` capture le panneau avec des données fictives ; sans `JAUGE_DEMO`, avec tes vrais chiffres.
