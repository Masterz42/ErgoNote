# Installer ErgoNote sous Linux

Deux facons, selon ce que vous avez telecharge.

## 1. Le paquet .deb (le plus simple)

Fichier : `dist/linux/ergonote_<version>_amd64.deb`

1. Double-cliquez sur le `.deb`.
2. Le gestionnaire de logiciels s'ouvre (GDebi, Discover ou aptinstall).
3. Cliquez sur Installer, entrez votre mot de passe.
4. ErgoNote apparait dans le menu, rubrique Office ou Education.

En terminal, la meme chose :

```bash
sudo apt install ./dist/linux/ergonote_1.6.0_amd64.deb
```

## 2. L'archive portable, sans root

Fichier : `dist/linux/ErgoNote-v<version>-Linux-x86_64.tar.gz`

1. Clic droit, Extraire ici.
2. Ouvrez le dossier extrait.
3. Double-cliquez sur `install-mon-cartable.sh` et choisissez Executer. Si votre
   gestionnaire de fichiers ouvre le script dans un editeur, utilisez le terminal :
   clic droit dans le dossier, Ouvrir un terminal, puis la commande ci-dessous.

   ```bash
   bash install-mon-cartable.sh --lancer
   ```

Le programme va dans `~/.local/bin/ergonote`, une entree est ajoutee au menu et
un raccourci `ErgoNote` est pose sur le Bureau. Aucun mot de passe n'est demande.

Pour lancer sans rien installer, double-cliquez sur `ergonote` dans le dossier
extrait. Gardez le dossier entier : le carnet d'accueil est dans `demo/`.

## Pourquoi double-cliquer sur le binaire seul ne fait rien

Un fichier programme Linux n'a pas d'entree de menu ni d'icone. Selon le
gestionnaire de fichiers, un double-clic l'ouvre dans un editeur de texte, demande
une confirmation, ou ne fait rien. C'est pour cela que le paquet et l'archive
fournissent un installateur : ils creent le raccourci que le systeme sait lancer.

Si le raccourci du Bureau ne s'ouvre pas : clic droit dessus, puis
Autoriser le lancement (Cinnamon et GNOME demandent cette confirmation une fois).

## Ou vont les fichiers

| Element | Emplacement |
|---|---|
| Programme (installation utilisateur) | `~/.local/bin/ergonote` |
| Entree de menu | `~/.local/share/applications/com.ergonote.app.desktop` |
| Icones | `~/.local/share/icons/hicolor/*/apps/com.ergonote.app.png` |
| Carnet d'accueil | `~/.local/share/ergonote/demo/welcome-to-ergonote` |
| Vos carnets | `~/Documents/ErgoNote` (ou le dossier deja memorise) |

Une mise a jour ne supprime jamais `~/Documents/ErgoNote`.

## Desinstaller

Installation utilisateur :

```bash
rm -f ~/.local/bin/ergonote
rm -f ~/.local/share/applications/com.ergonote.app.desktop
rm -rf ~/.local/share/ergonote/demo/welcome-to-ergonote
rm -f ~/Bureau/ErgoNote.desktop
```

Paquet .deb :

```bash
sudo apt remove ergonote
```

## Reconstruire les fichiers depuis le depot

```bash
./creer-version-linux.sh
```

Le script compile la version release, ecrit le binaire
`ErgoNote-v<version>-Linux-x86_64`, le dossier portable, l'archive `.tar.gz`, le
paquet `.deb` et `dist/linux/SHA256SUMS`.
