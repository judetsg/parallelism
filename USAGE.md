# Guide d'usage — Mini-calculateur parallèle (Rust + rayon)

Ce document explique **à quoi sert ce programme** et **comment s'en servir**.
Le README, lui, explique le code ligne par ligne. Les deux se complètent.

---

## 1. À quoi sert ce programme ?

### Le problème de départ

Un ordinateur moderne a plusieurs cœurs : il peut faire **plusieurs calculs en
même temps**. Mais un programme n'en profite pas tout seul — par défaut, Rust
(comme la plupart des langages) exécute ton calcul sur **un seul cœur**, et les
autres attendent. Sur une machine de 8 cœurs, tu utilises alors 1/8 de la
puissance disponible.

Le programme rend ce gain **visible et vérifiable** : chaque calcul est exécuté
deux fois, une fois par un seul cœur, une fois par N cœurs, puis les deux temps
sont affichés côte à côte.

### Les trois questions auxquelles il répond

1. **Est-ce que le parallèle aide vraiment ?** → le `gain` affiché.
2. **Est-ce que ça donne le même résultat ?** → la ligne `controle`, qui doit
   toujours finir par `true`.
3. **Dans quels cas ça n'aide pas ?** → en réglant les threads ou le nombre de
   répétitions (voir §6), on le constate soi-même.

### Ce qu'il n'est pas

- Ce n'est **pas** un outil de mesure scientifique : pas de contrôle
  statistique, pas de protocole reproductible, les durées sont prises sur une
  machine qui fait autre chose en même temps (navigateur, messagerie…).
- Ce n'est **pas** un outil de production : les données sont fabriquées à la
  volée, rien n'est lu sur le disque, rien n'est écrit.
- Ce n'est **pas** un tutoriel de `rayon` complet : il montre 5 cas typiques,
  pas toute la bibliothèque.

C'est un **outil pédagogique** : le plus simple possible pour comprendre ce
qu'est le parallélisme, le voir, et le mesurer honnêtement.

---

## 2. Les 5 démonstrations

| # | Démonstration | Ce que ça imite | Le patron rayon | Contrôle affiché |
|---|---|---|---|---|
| 1 | **Addition de vecteurs** (1 M) | traiter une image ou un tableau de mesures | `par_iter().zip().for_each()` — écriture en place | `c[100] = 300`, `identique ? true` |
| 2 | **Produit scalaire** (1 M) | noter / facturer des lignes | `par_iter().map().sum()` — réduction | `seq = par = 2000000` |
| 3 | **Pi par rectangles** (20 M) | estimer une probabilité par simulation | `into_par_iter().map().sum()` sur un intervalle | `3.141593` des deux côtés |
| 4 | **Tri d'un tableau** (1 M) | trier une colonne de tableur | `par_sort_unstable()` — rayon découpe, trie, fusionne | `trie ? true`, `identique ? true` |
| 5 | **Compression d'un fichier** (64 Mo) | « zipper » un fichier avant de l'envoyer | `par_iter().map().collect()` sur des blocs | `64 Mo -> 6 Mo`, `reconstitué intact ? true` |

Elles couvrent volontairement des situations différentes :

- **chaque élément indépendant** (1) ;
- **réduction : une seule somme à partagé** (2 et 3) — c'est là que se cache
  le piège de l'écriture concurrente ;
- **une opération que rayon fait tout seul** (4) ;
- **un traitement qu'on découpe soi-même en blocs** (5).

---

## 3. Démarrer le programme

```bash
cd parallelism
cargo run --release
```

Trois règles à connaître avant de lancer :

1. **`--release` est obligatoire pour une mesure crédible.** En mode debug,
   le tri prend ~2,5 s au lieu de ~40 ms : on mesure alors le coût du code non
   optimisé, pas le parallélisme.
2. **Il faut un vrai terminal.** L'interface est une TUI : si la sortie est
   redirigée (`printf 'l' | cargo run`), le programme s'arrête avec un message
   explicite plutôt que de planter.
3. **La première compilation prend une à deux minutes** (cursive + crossterm).
   Les suivants sont immédiats.

---

## 4. L'interface, écran par écran

### 4.1 L'écran de menu

Trois réglages, lus dans l'ordre :

1. **Démonstration** — liste déroulante des 5 calculs (↑ ↓ puis Entrée).
2. **Nombre de threads** — de 1 à deux fois le nombre de cœurs détectés.
   Par défaut : le nombre de cœurs (8 sur cette machine).
3. **Répétitions par variante** — combien de fois chaque version est mesurée.
   Par défaut 5, acceptées de 1 à 50.

Une ligne de résumé (`Choix actuel : … — 8 threads`) rappelle la sélection :
indispensable, car la liste des threads est plus longue que ce qu'on voit à
l'écran et la ligne sélectionnée peut être hors champ.

### 4.2 Lancer

| Touche | Effet |
|---|---|
| `l` | lance la mesure (le plus simple) |
| `Entrée` sur le bouton | idem |
| `↑` `↓` + `Entrée` | choisir une valeur dans une liste |
| `Tab` | aller au champ répétitions, puis au bouton |
| `q` | quitter |

### 4.3 L'écran de mesure

Pendant le calcul, deux indications avancent :

- le libellé d'étape : `sequentiel 3/5`, puis `parallele 2/5` ;
- une barre de progression avec un pourcentage.

Le calcul se fait dans un **thread séparé** : l'interface reste utilisable, on
peut quitter à tout moment.

Quand c'est terminé, le rapport s'affiche en haut :

```
 seq  : min   48.1 ms  mediane   67.8 ms  max   75.3 ms
 par  : min   35.0 ms  mediane   38.0 ms  max   38.3 ms
 gain : x1.78 (seq / par, sur la mediane)
 controle : c[100] = 300 (attendu 300) | identique ? true
```

| Touche | Effet |
|---|---|
| `Entrée` | retour au menu (si une mesure est en cours, elle est abandonnée) |
| `q` | quitter le programme |

---

## 5. Lire le rapport

### `min`, `mediane`, `max`

Une mesure unique n'est jamais fiable : un autre processus, un ventilateur, un
changement de fréquence suffisent à la déformer. Le programme répète donc le
calcul et affiche les trois extrêmes :

- **`min`** — le meilleur cas, peu représentatif (tout s'est bien passé) ;
- **`max`** — le pire cas, souvent une interruption ;
- **`mediane`** — la valeur centrale, **c'est elle qui sert au gain**.

Passer de 1 à 10 répétitions change rarement le `gain` affiché, mais resserre
nettement `min` et `max` : c'est le but.

### `gain`

```
gain = médiane(seq) / médiane(par)
```

Un `x1.78` veut dire : *le parallèle a mis 1,78 fois moins de temps*.

Valeurs observées sur 8 cœurs : **de x1.8 à x4.8**.

> **Ne vous attendez jamais à x8.** Un facteur 8 exige un travail parfaitement
> divisible et aucun coût parasite. En pratique, on perd sur : la création du
> pool de threads à chaque mesure, la mémoire (bande passante partagée), les
> irrégularités du système. C'est normal, et c'est une partie de la leçon.

### `controle`

La ligne qui compte le plus : elle compare le résultat parallèle au résultat
séquentiel (ou à une valeur de référence connue).

**Elle doit toujours finir par `true`.** Un `false` signifierait un vrai bug de
parallélisme — typiquement deux threads écrivant au même endroit. Si tu vois
`false`, note la démonstration et le nombre de threads, et change de
nombre de threads pour voir si c'est systématique.

---

## 6. Expériences à faire (c'est là que ça devient intéressant)

### 6.1 La courbe du nombre de threads

Lancer la même démonstration avec **1, 2, 4, 8 puis 16 threads** :

- **1 thread** : le parallèle est *plus lent* que le séquentiel (coût de création
  du pool, aucun travail à partager) ;
- **2 threads** : petit gain ;
- **8 threads** : le meilleur rapport ;
- **16 threads** (2× les cœurs) : le gain retombe, les threads se marchent
  dessus.

C'est la démonstration la plus parlante : **le parallélisme n'est pas « plus
de threads = plus rapide »**.

### 6.2 1 répétition contre 20

Passer de 1 à 20 répétitions : le `gain` se stabilise, `min` et `max` se
resserrent. On comprend pourquoi une mesure isolée ne veut rien dire.

### 6.3 Charger la machine pendant la mesure

Lancer une mesure, puis ouvrir un gros fichier dans le navigateur pendant
qu'elle tourne : le `max` explose. Le `gain` calculé sur la médiane bouge peu —
c'est exactement la robustesse recherchée.

### 6.4 Observer les rapports entre démonstrations

| Démonstration | Comportement observé | Enseigne |
|---|---|---|
| Addition | le plus petit gain | limitée par la bande passante mémoire, pas par les cœurs |
| Compression | le plus gros gain | 64 blocs totalement indépendants |
| Tri | gain moyen | rayon découpe et fusionne, mais la fusion est sequential |

### 6.5 `--release` contre debug

Lancer la démo « Tri » avec et sans `--release` : ~30× d'écart. Rappel que le
gain du parallélisme ne se voit correctement que sur du code optimisé.

---

## 7. Limites connues

- **Dépend de la charge machine.** Sur un poste déjà occupé, `min`/`max` sont
  larges. C'est inhérent à toute mesure de temps.
- **Mémoire** : la démo Compression travaille sur ~64 Mo de données et
  construit plusieurs copies (≈ 150 Mo). Sur une machine très contrainte, la
  réduire en passant `BLOCS`/`taille` dans `run_compression()`.
- **`1 thread` n'est pas l'égalité** : le mode parallèle ajoute le coût de
  création du pool, qui est ici **inclus dans la mesure** (c'est volontaire :
  c'est le coût réel).
- **Terminal d'environ 80×30 minimum** : le menu occupe ~25 lignes.
- **Pas de mode headless** : impossible de l'utiliser dans une CI ou un script
  (une TUI a besoin d'un terminal). Pour une utilisation automatisée, il
  faudrait réécrire une sortie texte, ce qui n'est pas l'objectif ici.

---

## 8. Si quelque chose cloche

| Symptôme | Cause probable | Solution |
|---|---|---|
| « Ce programme est une interface TUI… » | sortie redirigée, lancement dans un script | lancer directement dans un terminal |
| L'interface semble figée | démo Tri en mode debug (2,5 s × 10 passages) | relancer avec `--release`, ou patience |
| `Ctrl+C` semble sans effet | le processus est dans une boucle de calcul | `Ctrl+C` marche, sinon `kill` depuis un autre terminal |
| La ligne `controle` affiche `false` | **bug de parallélisme** | changer le nombre de threads, noter le cas, le signaler |
| Le gain est proche de x1.00 | charge memory-bound (démo 1) ou 1 thread | normale pour l'addition ; essayer une autre démo |
| L'écran est illisible | terminal trop petit | agrandir la fenêtre (80×30 minimum) |

---

## Voir aussi

- **`README.md`** — le code expliqué ligne par ligne, section par section.
- `Cargo.toml` — les deux dépendances : `rayon` (le parallélisme) et
  `cursive` (l'interface).