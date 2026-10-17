En prenant en compte le contexte d'utilisation de visugps2 , tu vas me proposer un plan pour l'ajout d'une fonctionnalité permettant de détecter les communes traversées par une trace GPX. 

Cette fonctionnalité devra être déclenchée automatiquement aprés que le fichier gpx **ait été nettoyé**. Si le circcuit importé est propre la fonctionnalité est lancée automatiquement. Si la tâche de fond décrite plus bas dans le prompt est déjà en cours on attend la fin de cette tâche. Cette tâche de fond lancera automatiquement l'analyse des communes pour le fichier gpx en cours. 

**Seul un fichier json des communes devra être généré et stocké** dans le répertoire dedié du contexte d'exécution dans l'arborescence de la trace traitée.

Au lancement de l'application une tache de fond devra être lancée pour s'assurer que l'ajout des données de la feature "communes " soit bien réalisé pour l'ensemble des circuits qui ont déjà été nettoyés dans le passé. Si tout est bon on ne fait rien et on stope la tâche. 

le fichier json "traces.json" situé dans le répertoire dedié du contexte d'exécution devra géré l'état de la feature. 
Ce fichier json contiendra l'information suivante : "communes_completed" : true ou false. Si cette information n'est pas présente on considère que la tâche n'a pas été réalisée.
Ce fichier json devra être mis à jour : 
- à la fin du traitement de la feature "communes " pour le fichier gpx en cours
- lorsque l'utilisateur ajoute une fichier gpx qui n'a pas encore été traité

Dans le composant Circuit.vue le bouton Editer le domicile devra être affiché : 
- au survol du composant en gris si le circuit n'est pas clean. Le bouton ne doit pas déclanccher d'action. 
- au survol en orange si le cicuit est clean et que la fonctionnalité est en cours de traitement ou si la tache de fond est active. Le bouton ne doit pas déclanccher d'action.
- au survol en vert si toutes les communes traversées ont été détectées. Le bouton ne doit pas déclanccher d'action.
- Dans tous les autres cas il doit être en affichage forcé en rouge et un clique sur le bouton lance la fonctionnalité uniquement pour ce circuit. 