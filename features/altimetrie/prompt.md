En prenant en compte le contexte d'utilisation de visugps2 , tu vas me proposer un plan pour l'ajout d'une fonctionnalité permettant de :
- lisser le profile alitimétrque de la trace,
- détecter les segments de pentes,
- détecter les montées
- générer un fichier altimetrie.json

Pour cela tu trouveras dans le dossier features/altimetrie plusieurs fichiers qui ont permis de prototyper une solution technique, mais ils ne font pas partie de l'architecture logicielle. Il va falloir les intégrer dans l'architecture logicielle, en respectant les standards de l'application.
Les algorithmes développés dans ce dossier ont été approuvés et ont montré leur pertinence par rapport à notre besoin.

Cette fonctionnalité devra être déclenchée automatiquement aprés que le fichier gpx **ait été nettoyé**. Si le circcuit importé est propre la fonctionnalité est lancée automatiquement.

**Seul un fichier altimetrie.json devra être généré et stocké** dans le répertoire dedié du contexte d'exécution dans l'arborescence de la trace traitée.

Après que le fichier altimetrie.json ait été généré, nous devons pouvoir modifier/ajouter/Supprimer des montées.
Pour cela à partir de l'icône mdi-terrain du composant circuit.vue, on ouvrira une nouvelle vue altimetrie.vue qui devra avoir une ihm équivalente à celle des autres vues avec :
- une tollbar avec :
- un bouton pour ouvrir le volet des paramètres de configuration,
- un bouton pour fermer la vue,
- une colonne à gauche qui affiche les montées détectées,
- une colonne à droite rectractée par défaut pour éditer les paramètres de configuration.

Dans la colonne de gauche on affichera chacune des montées détectées avec :
- une représentation graphique du positionnement de la montée par rapport à la trace complète
- la distance depuis le km 0 du point de vallée
- la distance totale de la montée
- le dénivelé total de la montée
- la pente moyenne de la montée
- un bouton pour pouvoir supprimer la montée
- dans le cas des montées fusionnées un bouton pour pouvoir les séparer

Les montées fusionnées devront pouvoir etre séparées via un bouton, et les montées séparées devront pouvoir etre fusionnées.

A la suite de l'affichage des montées on ajoutera un bouton "créer une montée" pour pouvoir créer une nouvelle montée manuellement.
Suite au clique sur ce bouton le point de vallée sera créé en cliquant sur le profile altimétrique, en dehors d'une montée existante, puis un second clique sur le graphique permettre de créer le point sommet. Une fenêtre d'aide sera affichée pour guider l'utilisateur.

Dans la colonne de droite des paramètres on retrouve la possibilité d'ajuster les paramètres de configuration de l'alimétrie. On reprendra la même représentation graphique que nous avons sur les autres vues de l'application.
La synthèse des paramètres de configuration devra être affichée sur la partie suppérieure en ajustant la couleur de l'icône paramètre de la zone titre.
On affichera en dessous en tiny la ou les causes qui ont mené à cette synthèse dégradée

Au centre on affichera la carte mapbox en 3D avec la trace colorée en fonctions des segments de pente.

**Le widget altimétrie**

Dans un widget sous la forme d'un div flotant situé en bas de la fenêtre on affichera un graphiques interactif représentant le profile altimétrique de la trace coloré en fonctions des segements de pente.
Sur ce graphique on devra pouvoir :
- avoir sur l'axe Y des repères sur l'altitude avec un trait pointillé le long du graphe, on affichera par defaut 10 m/px, avec une plage allant de 2,5 m a 50 m par pas de 2,5m
- avoir sur l'axe X des repères sur la distance. Par defaut on affichera 50m par pixel paramétrable de 5 à 100 m, par pas de 5 m.
- une visualisation des points de vallée et de sommet via un triangle orienté vers le bas en haut du graphe, et un trait pointillé verticale descendant jusqu'à l'axe des x. ce triangle et le trait pointillé seront Vert pour les points de vallée et rouge pour les points de sommet.
- les triangles étants cliquables pour déplacer via drag and drop les points de vallée et de sommet. Lors du clique l'échelle des x doit être ajustée à x m/pixel. 12,5 m par défaut et compris entre 2,5m et 50 m par pas de 2.5 m.

- une visualisation des montées via un trait horizontal situé au niveau de l'axe des X, coloré en fonction de la catégorie de la montée. Un simple clic sur un de ces traits doit afficher sur la carte la montée correspondante.

Un simple clic dans ce widget doit recentrer la carte sur ce point
Si le focus de la souris est sur ce widget la molette doit permettre de déplacer la scrollbar horizontalement.
Un ou plusieurs exemples de représentation graphique sont présents dans le dossier features/altimetrie/.

Aprés avoir ajouté, modifié, supprimé des montées, il faudra pouvoir sauvegarder les modifications.
Si on tente de quitter la vue alors q'une modification est en cours il faudra mettre un message d'attention à l'utilisateur

Le clé mapbox n'a pas besoin d'être présente dans les paramètres alimétrie, elle est déjà présente dans l'application.

tu dois me proposer un plan d'implémentation, en ayant en tête d'apporter le moins de modification possible à l'architecture logicielle de l'application et en respectant les standards de l'application.

Si tu as des points de doute tu devras me challenger en me questionnant sur ces points