# Mini-calculateur parallèle — Rust + rayon (expliqué ligne par ligne)

Programme console **piloté par une interface TUI** : 5 calculs simples,
chacun en version séquentielle (1 thread) puis parallèle (N threads avec rayon),
**mesuré plusieurs fois** (min / médiane / max) pour prouver le gain.

> 📖 **[USAGE.md](USAGE.md)** — à quoi sert ce programme et comment s'en servir :
> écran par écran, tableau des touches, lecture du rapport, expériences à faire,
> limites connues et dépannage. Ce README, lui, explique le **code** ligne par
> ligne.

## 1. Installer et lancer

```bash
rustc --version   # doit afficher une version, sinon installer via https://rustup.rs/
cargo --version
cd parallelism
cargo run --release   # IMPORTANT : --release, sinon le tri est ~30x plus lent
```

Les dépendances (`rayon`, `cursive`) se téléchargent seules au premier run.

> ⚠️ Le programme est une **interface TUI** : il lui faut un vrai terminal.
> Si la sortie est redirigée (`printf 'l' | cargo run`), il s'arrête avec un
> message explicite au lieu de planter.

L'interface est un vrai menu dans le terminal :

```
 Mini-calculateur parallele : Rust + rayon
 Coeurs detectes : 8
 1. Demonstration
┌─────────────────────────────────────────────┐
│1. Addition de vecteurs (1 M)                 │
│2. Produit scalaire (1 M)                     │
│3. Pi par rectangles (20 M)                   │
│4. Tri d'un tableau (1 M)                     │
│5. Compression d'un fichier (64 Mo)           │
└─────────────────────────────────────────────┘
 2. Nombre de threads          <- flèches + Entrée
┌─────────────────────────────────────────────┐
│1 thread(s)                                 ▒│
│2 thread(s)                                 ││
│3 thread(s)                                 ││
└─────────────────────────────────────────────┘
 3. Repetitions par variante   <- 5 par défaut
 Choix actuel : 1. Addition de vecteurs (1 M) — 8 threads
                       < Lancer la mesure >
 l = lancer la mesure    fleches + Entree = choisir une valeur
 Astuce : cargo run --release, sinon le tri est ~30x plus lent
```

Pendant la mesure, une barre de progression avance puis le rapport s'affiche :

```
 Entree = retour au menu    q = quitter
 seq  : min  302.4 ms  mediane  317.0 ms  max  345.1 ms
 par  : min   69.6 ms  mediane   70.1 ms  max  162.8 ms
 gain : x4.52 (seq / par, sur la mediane)
 controle : 64 Mo -> 6 Mo (x10.3) | identique ? true | reconstitue intact ? true

 5. Compression d'un fichier (64 Mo)
 64 blocs de 1 Mo, compresses separement puis recolles
 8 threads | 5 repetitions par variante
sequentiel 3/5
██████████▊          27 %
```

**Mode d'emploi**

| Touche | Menu | Écran de mesure |
|---|---|---|
| `↑` `↓` | choisir une valeur dans une liste | — |
| `Entrée` | valider la valeur sélectionnée | **retour au menu** (la mesure est abandonnée) |
| `Tab` | passer au champ / au bouton | — |
| `l` | **lancer la mesure** | — |
| `q` | quitter le programme | quitter le programme |

Le nombre de répétitions est borné entre 1 et 50 : au-delà, la mesure dure
plus longtemps pour une précision qui n'augmente plus vraiment.

Mesures réelles sur 8 cœurs (release, 5 répétitions) : les gains vont de **x1.8**
pour l'addition (limitée par la mémoire) à **x4.8** pour la compression, en
passant par x2.1 pour le tri et x4.5 pour Pi. Les chiffres varient selon la
charge de la machine : c'est exactement pour ça qu'on répète les mesures.

## 2. Fichiers

```
Cargo.toml  -> rayon + cursive (cursive = l'interface TUI)
src/main.rs -> tout le programme, ~700 lignes commentées
```

## 3. Imports `src/main.rs:6-17`, ligne par ligne

```rust
use cursive::event::{Event, Key};     // :6 evenements clavier
use cursive::theme::Effect;           // :7 effets d'affichage (gras, gris)
use cursive::view::{Nameable, Resizable}; // :8 nommer une vue, fixer sa taille
use cursive::views::{Button, ...};    // :9 les briques de l'interface
use cursive::Cursive;                 // :10 l'application TUI
use std::io::IsTerminal;              // :11 verifier qu'on a un vrai terminal
use rayon::prelude::*;                // :12 par_iter, sum, par_sort_unstable...
use rayon::ThreadPoolBuilder;         // :13 pool a N threads
use std::sync::atomic::{AtomicBool, Ordering}; // :14 annulation
use std::sync::mpsc::{self, Sender};  // :15 canal worker -> interface
use std::sync::Mutex;                 // :16 le canal doit etre partageable
use std::time::{Duration, Instant};   // :17 chrono
```

- `IsTerminal` (`:11`) évite le panic de crossterm quand la sortie est
  redirigée : on affiche un message lisible et on sort avec le code 1.

## 4. `nb_threads()` `src/main.rs:19`

```rust
std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
```

- Demande au système le nb de coeurs logiques (ex: 8).
- Le menu propose `1 .. nombre de coeurs x2` pour voir le surcoût.

## 5. Addition vecteurs

### `vector_add_seq()` `src/main.rs:26` — référence 1 thread

```rust
for i in 0..a.len() { c[i] = a[i] + b[i]; }
```

- `a: &[f64]` = vue lecture, `c: &mut [f64]` = vue écriture.
- `i` va de 0 à n-1, ex: `c[100]=100+200=300`.

### `vector_add_par()` `src/main.rs:34` — version parallèle

```rust
a.par_iter()             // vue parallèle de a, rayon la découpe en chunks
  .zip(b.par_iter())     // paire (a[i], b[i]) pour chaque i
  .zip(c.par_iter_mut()) // c[i] en écriture : ((a[i],b[i]), c[i])
  .for_each(|((&x, &y), z)| { *z = x + y; });
```

- Chaque `c[i]` est écrit une seule fois par un seul thread → pas de conflit.
- Fin du `for_each` = barrière : on attend tous les threads.

## 6. Produit scalaire (reduction)

### `dot_product_seq()` `src/main.rs:46`

```rust
let mut sum = 0.0;
for i in 0..a.len() { sum += a[i] * b[i]; }
sum
```

### `dot_product_par()` `src/main.rs:56` — somme parallèle

```rust
a.par_iter().zip(b.par_iter()).map(|(&x, &y)| x * y).sum()
```

- Chaque thread calcule ses produits sur son morceau, `.sum()` additionne
  les sommes partielles à la fin.
- Si `sum` était partagé entre threads, 2 threads écriraient en même temps :
  la parade est « somme locale + somme finale ».

## 7. Pi par rectangles

Aire sous `4/(1+x²)` entre 0 et 1 = Pi. On coupe en `steps` rectangles.

### `pi_seq()` `src/main.rs:64`

```rust
let step = 1.0 / steps as f64;
for i in 0..steps {
  let x = (i as f64 + 0.5) * step;  // milieu du rectangle
  sum += 4.0 / (1.0 + x * x);       // hauteur
}
sum * step                          // aire = somme hauteurs × largeur
```

### `pi_par()` `src/main.rs:76`

```rust
(0..steps).into_par_iter()
  .map(|i| { let x = (i as f64+0.5)*step; 4.0/(1.0+x*x) })
  .sum::<f64>() * step
```

- Même formule que seq, seul le partage des `i` change.

## 8. Trier un gros tableau

Cas de la vie réelle : trier une colonne de 1 M de valeurs.

### `hashe()` `src/main.rs:101` + `genere_valeurs()` `src/main.rs:114`

```rust
fn hashe(mut x: u64) -> u64 {   // splitmix64, reproductible partout
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
fn genere_valeurs(n: usize) -> Vec<u32> {
    (0..n).map(|i| (hashe(i as u64) >> 32) as u32).collect()
}
```

- `hashe(i)` : même index → même valeur, donc les données sont identiques
  à chaque run.
- Permet de fabriquer du hasard **sans le crate `rand`**.

### `tri_seq()` `src/main.rs:121` / `tri_par()` `src/main.rs:127`

```rust
v.sort_unstable();          // 1 thread
v.par_sort_unstable();      // rayon découpe, trie, fusionne
```

- **Un seul appel** : pas de boucle à écrire, pas de résultat à recoller.
- Le meilleur exemple pour montrer qu'on n'a pas toujours à gérer les threads.

## 9. Compresser un gros fichier

Cas de la vie réelle : « zippe le fichier avant de l'envoyer ». 64 Mo coupés
en 64 blocs de 1 Mo, chaque bloc compressé indépendamment.

### `genere_fichier()` `src/main.rs:135` — le faux fichier source

```rust
while i < n {
  let h = hashe(i as u64);
  let longueur = (h % 40) as usize + 1;   // suite de 1 à 40 octets identiques
  let fin = (i + longueur).min(n);
  v.resize(fin, (h >> 8) as u8);
  i = fin;
}
```

### `compresse_rle()` `src/main.rs:150` — la compression d'UN bloc

```rust
while i < bloc.len() {
  let v = bloc[i];
  let mut k = 1;
  while i + k < bloc.len() && bloc[i + k] == v && k < 255 { k += 1; }
  out.push(k as u8);   // paire (longueur, valeur)
  out.push(v);
  i += k;
}
```

- Un bloc est compressible **tout seul** : c'est ce qui rend la compression
  parallélisable. Un seul flux ne le serait pas.

### `compresse_seq()` `src/main.rs:179` / `compresse_par()` `src/main.rs:185`

```rust
blocs.iter().map(|b| compresse_rle(b)).collect()      // seq
blocs.par_iter().map(|b| compresse_rle(b)).collect()  // par
```

- Le découpage `chunks()` est fait une fois pour les deux variantes, donc
  les octets compressés sont **identiques**.

## 10. Une variante = une mesure honnête

Chaque démonstration expose 2 variantes (`seq` et `par`) sous forme de
fonctions `run_*` qui **préparent leurs données puis ne chronomètrent que le
calcul** : sinon on mesurerait la création des tableaux.

### `run_addition()` `src/main.rs:196`

```rust
const PASSES: usize = 10;      // sinon le goulot est la mémoire, pas le CPU
let a: Vec<f64> = (0..n).map(|i| i as f64).collect();     // hors chrono
let mut c = vec![0.0; n];                                 // hors chrono
let t0 = Instant::now();                                  // chrono start
avec_pool(n_threads, || { for _ in 0..PASSES { vector_add_par(&a,&b,&mut c); } });
let d = t0.elapsed();                                      // chrono stop
```

- **Leçon** : une simple addition de 1 M de nombres ne gagne presque rien
  (x1.5) car elle est limitée par la bande passante mémoire. Avec 10 passages,
  on ajoute du calcul et le gain devient visible. Tout ne se parallélise pas
  aussi bien — c'est le message principal de cet exemple.

Les 5 fonctions : `run_addition` `:196`, `run_dot` `:225`, `run_pi` `:240`,
`run_tri` `:253`, `run_compression` `:269`.

## 11. Mesures : min / médiane / max

### `mesurer()` `src/main.rs:308`

```rust
for _ in 0..reps {
    if ANNULATION.load(Ordering::Relaxed) { return None; }
    durees.push(variante());
}
durees.sort();
Some(Stats { min: durees[0], mediane: durees[reps / 2], max: durees[reps - 1] })
```

- Une seule mesure est du bruit (un autre processus, un turbo CPU...). On
  répète 5 fois par défaut et on affiche min / médiane / max.
- La **médiane** sert au calcul du gain : elle ignore les pics parasites.
- `ANNULATION` `src/main.rs:288` permet de quitter l'écran sans attendre un
  résultat qui ne sera plus affiché.

### `formater()` `src/main.rs:327` — le rapport affiché

```
 seq  : min  302.4 ms  mediane  317.0 ms  max  345.1 ms
 par  : min   69.6 ms  mediane   70.1 ms  max  162.8 ms
 gain : x4.52 (seq / par, sur la mediane)
```

- `gain = mediane_seq / mediane_par` : x4.52 veut dire « le parallèle a mis
  4,52 fois moins de temps ».

### `Demo::controle()` `src/main.rs:416` — la vérification

- Chaque démo compare sa sortie seq et sa sortie par (ou donne une valeur
  de référence : `c[100] = 300`, `3.141593`, aller-retour de compression).
- **Un contrôle `false` = bug de parallélisme.** Ici, toujours `true`.

## 12. L'interface TUI

### `lancer()` `src/main.rs:465` — le worker

```rust
let seq = match mesurer(reps, || {
    let d = demo.run(n_threads, false).0;   // 1 répétition seq
    let _ = tx.send(Etape::Avance { texte: ..., fait, total });  // previent l'écran
    d
}) { ... };
```

- Tout le calcul se fait dans **un thread à part** : l'interface ne freeze
  jamais.
- 3 phases : `reps` fois en seq, `reps` fois en par, puis 1 contrôle.

### `maj_ecran()` `src/main.rs:521` — l'affichage

```rust
Etape::Avance { texte, fait, total } => {
    siv.call_on_name("etape", |v: &mut TextView| v.set_content(texte));
    siv.call_on_name("barre", |v: &mut ProgressBar| v.set_value((fait*100)/total));
}
```

- `call_on_name("barre", ...)` retrouve la vue par son nom et la modifie.
- ⚠️ Une vue ne doit être nommée **qu'une seule fois** : deux `with_name`
  imbriqués rendent le `call_on_name` incapable de retrouver le bon type, et
  la mise à jour est silencieusement ignorée.
- La barre est dessinée par cursive en **vidéo inverse** (une bande colorée,
  pas des caractères `█`) : copier le terminal en texte brut la fait
  disparaître, il faut copier avec les attributs (`tmux capture-pane -e`).

### `ecran_mesure()` `src/main.rs:551` — l'écran de mesure

- `full_screen()` : l'écran couvre tout le terminal, sinon le menu reste
  visible en dessous et les deux se mélangent à l'écran.
- `OnEventView` intercepte `Entrée` (retour au menu, la mesure est annulée)
  et `q` (fermeture).
- La barre est limitée à 40 colonnes, sinon elle s'étire sur toute la largeur
  et son pourcentage part dans la marge de droite.

### `maj_resume()` `src/main.rs:583` et `ecran_menu()` `src/main.rs:617`

```rust
demos.on_select(|siv, _| maj_resume(siv));    // la liste des demos pilote le resume
threads.on_select(|siv, _| maj_resume(siv));  // la liste des threads aussi
```

- La liste des threads va de 1 à 2× le nombre de cœurs : la ligne sélectionnée
  n'est pas forcément visible. `on_select` met donc à jour une ligne de résumé
  (« Choix actuel : … — 8 threads »).
- `maj_resume` est une **fonction** et non une closure : `on_select` exige
  une closure `'static`, qui ne peut pas capturer une variable locale.

### `main()` `src/main.rs:659` — la boucle

```rust
siv.set_fps(30);   // 30 images/s : la barre bouge même sans frappe
siv.add_global_callback(Event::Refresh, move |siv| { /* vide le canal */ });
```

- Une **seule** callback globale écoute `Event::Refresh` (toutes les 1/30 s) :
  elle vide le canal `mpsc` et met la barre à jour.
- Le `Mutex` autour du `Receiver` est obligatoire : cursive exige des
  callbacks `Send + Sync`, et un `Receiver` seul n'est que `Send`.
- `IsTerminal` `src/main.rs:11` : sans vrai terminal, crossterm panique ; on
  teste donc `stdin().is_terminal()` et on affiche un message lisible.

## 13. Comment ça fonctionne (à réciter)

- `par_iter` découpe les données en morceaux, un par thread du pool.
- Chaque thread calcule sa partie avec ses variables locales.
- `.sum()` additionne les résultats partiels à la fin.
- Pour un **tri**, rayon fait tout : il découpe, trie, puis fusionne.
- Pour un **gros fichier**, c'est nous qui le découpons en blocs, parce que
  rayon ne peut pas inventer un découpage qui a du sens pour le problème.
- Pour l'**interface**, le calcul vit dans un thread et parle à l'écran
  par un canal — jamais l'inverse.

## 14. Questions pièges

- **Conflit d'écriture où ?** Dans `dot` / `pi` si `sum` était partagé.
  Évitée par sommes locales + `.sum()`.
- **Pourquoi mesurer 5 fois ?** Une seule mesure est du bruit. Min, médiane
  et max le montrent, et la médiane donne le gain le plus fiable.
- **Pourquoi préparer les données hors du chrono ?** Sinon on mesure
  l'allocation mémoire, pas le calcul.
- **Pourquoi l'addition ne gagne-t-elle que x1.5 ?** Elle est limitée par la
  bande passante mémoire. Avec 10 passages (`PASSES`) on ajoute du calcul et
  le gain devient visible.
- **Pourquoi la compression marche ?** Parce qu'aucun bloc ne dépend d'un
  autre. Un seul flux de compression ne serait pas découpable.
- **Parfois par plus lent ?** Petit N : coût threads > gain. 1 thread est
  toujours plus lent que le seq. Teste avec 1, 2, 4, 8, 16.
- **En debug ?** Le tri prend ~30x plus longtemps : mesure toujours avec
  `cargo run --release`.