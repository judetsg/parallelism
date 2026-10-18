# Mini-calculateur parallèle — Rust + rayon (expliqué ligne par ligne)

Programme console **piloté par une interface TUI** : 5 calculs simples,
chacun en version séquentielle (1 thread) puis parallèle (N threads avec rayon),
**mesuré plusieurs fois** (min / médiane / max) pour prouver le gain.

## 1. Installer et lancer

```bash
rustc --version   # doit afficher une version, sinon installer via https://rustup.rs/
cargo --version
cd parallelism
cargo run --release   # IMPORTANT : --release, sinon le tri est ~30x plus lent
```

Les dépendances (`rayon`, `cursive`) se téléchargent seules au premier run.

L'interface est un vrai menu dans le terminal :

```
 Mini-calculateur parallele : Rust + rayon
 Coeurs detectes : 8
 1. Demonstration
 +-------------------------------------------+
 | 1. Addition de vecteurs (1 M)             |
 | 2. Produit scalaire (1 M)                 |
 | 3. Pi par rectangles (20 M)               |
 | 4. Tri d'un tableau (1 M)                 |
 | 5. Compression d'un fichier (64 Mo)       |
 +-------------------------------------------+
 2. Nombre de threads          <- flèches + Entrée
 3. Repetitions par variante   <- 5 par défaut
 [ Lancer la mesure ]          <- clic, ou touche l

 l = lancer    fleches + Entree = choisir une valeur    q = quitter
```

Pendant la mesure, une barre de progression avance puis le rapport s'affiche :

```
 Termine.
 seq  : min    39.6 ms  mediane    45.4 ms  max    74.2 ms
 par  : min    29.8 ms  mediane    31.2 ms  max    37.6 ms
 gain : x1.46 (seq / par, sur la mediane)
 controle : c[100] = 300 (attendu 300) | identique ? true
```

Mesures réelles sur 8 cœurs (release, 5 répétitions, les chiffres varient selon
la charge machine) : Addition x1.5, Scalaire x2.8, Pi x4.5, Tri x2.3,
Compression x4.7.

## 2. Fichiers

```
Cargo.toml  -> rayon + cursive (cursive = l'interface TUI)
src/main.rs -> tout le programme, ~670 lignes commentées
```

## 3. Imports `src/main.rs:6-16`, ligne par ligne

```rust
use cursive::event::{Event, Key};     // :6 evenements clavier
use cursive::theme::Effect;           // :7 effets d'affichage (gras, gris)
use cursive::view::{Nameable, Resizable}; // :8 nommer une vue, fixer sa taille
use cursive::views::{Button, ...};    // :9 les briques de l'interface
use cursive::Cursive;                 // :10 l'application TUI
use rayon::prelude::*;                // :11 par_iter, sum, par_sort_unstable...
use rayon::ThreadPoolBuilder;         // :12 pool a N threads
use std::sync::atomic::{AtomicBool, Ordering}; // :13 annulation
use std::sync::mpsc::{self, Sender};  // :14 canal worker -> interface
use std::sync::Mutex;                 // :15 le canal doit etre partageable
use std::time::{Duration, Instant};   // :16 chrono
```

## 4. `nb_threads()` `src/main.rs:18`

```rust
std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
```

- Demande au système le nb de coeurs logiques (ex: 8).
- Le menu propose `1 .. nombre de coeurs x2` pour voir le surcoût.

## 5. Addition vecteurs

### `vector_add_seq()` `src/main.rs:25` — référence 1 thread

```rust
for i in 0..a.len() { c[i] = a[i] + b[i]; }
```

- `a: &[f64]` = vue lecture, `c: &mut [f64]` = vue écriture.
- `i` va de 0 à n-1, ex: `c[100]=100+200=300`.

### `vector_add_par()` `src/main.rs:33` — version parallèle

```rust
a.par_iter()             // vue parallèle de a, rayon la découpe en chunks
  .zip(b.par_iter())     // paire (a[i], b[i]) pour chaque i
  .zip(c.par_iter_mut()) // c[i] en écriture : ((a[i],b[i]), c[i])
  .for_each(|((&x, &y), z)| { *z = x + y; });
```

- Chaque `c[i]` est écrit une seule fois par un seul thread → pas de conflit.
- Fin du `for_each` = barrière : on attend tous les threads.

## 6. Produit scalaire (reduction)

### `dot_product_seq()` `src/main.rs:45`

```rust
let mut sum = 0.0;
for i in 0..a.len() { sum += a[i] * b[i]; }
sum
```

### `dot_product_par()` `src/main.rs:55` — somme parallèle

```rust
a.par_iter().zip(b.par_iter()).map(|(&x, &y)| x * y).sum()
```

- Chaque thread calcule ses produits sur son morceau, `.sum()` additionne
  les sommes partielles à la fin.
- Si `sum` était partagé entre threads, 2 threads écriraient en même temps :
  la parade est « somme locale + somme finale ».

## 7. Pi par rectangles

Aire sous `4/(1+x²)` entre 0 et 1 = Pi. On coupe en `steps` rectangles.

### `pi_seq()` `src/main.rs:63`

```rust
let step = 1.0 / steps as f64;
for i in 0..steps {
  let x = (i as f64 + 0.5) * step;  // milieu du rectangle
  sum += 4.0 / (1.0 + x * x);       // hauteur
}
sum * step                          // aire = somme hauteurs × largeur
```

### `pi_par()` `src/main.rs:75`

```rust
(0..steps).into_par_iter()
  .map(|i| { let x = (i as f64+0.5)*step; 4.0/(1.0+x*x) })
  .sum::<f64>() * step
```

- Même formule que seq, seul le partage des `i` change.

## 8. Trier un gros tableau

Cas de la vie réelle : trier une colonne de 1 M de valeurs.

### `hashe()` `src/main.rs:100` + `genere_valeurs()` `src/main.rs:113`

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

### `tri_seq()` `src/main.rs:120` / `tri_par()` `src/main.rs:126`

```rust
v.sort_unstable();          // 1 thread
v.par_sort_unstable();      // rayon découpe, trie, fusionne
```

- **Un seul appel** : pas de boucle à écrire, pas de résultat à recoller.
- Le meilleur exemple pour montrer qu'on n'a pas toujours à gérer les threads.

## 9. Compresser un gros fichier

Cas de la vie réelle : « zippe le fichier avant de l'envoyer ». 64 Mo coupés
en 64 blocs de 1 Mo, chaque bloc compressé indépendamment.

### `genere_fichier()` `src/main.rs:134` — le faux fichier source

```rust
while i < n {
  let h = hashe(i as u64);
  let longueur = (h % 40) as usize + 1;   // suite de 1 à 40 octets identiques
  let fin = (i + longueur).min(n);
  v.resize(fin, (h >> 8) as u8);
  i = fin;
}
```

### `compresse_rle()` `src/main.rs:149` — la compression d'UN bloc

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

### `compresse_seq()` `src/main.rs:178` / `compresse_par()` `src/main.rs:184`

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

### `run_addition()` `src/main.rs:195`

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

Les 5 fonctions : `run_addition` `:195`, `run_dot` `:224`, `run_pi` `:239`,
`run_tri` `:252`, `run_compression` `:268`.

## 11. Mesures : min / médiane / max

### `mesurer()` `src/main.rs:307`

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
- `ANNULATION` `src/main.rs:287` permet de quitter l'écran sans attendre un
  résultat qui ne sera plus affiché.

### `formater()` `src/main.rs:326` — le rapport affiché

```
 seq  : min    39.6 ms  mediane    45.4 ms  max    74.2 ms
 par  : min    29.8 ms  mediane    31.2 ms  max    37.6 ms
 gain : x1.46 (seq / par, sur la mediane)
```

- `gain = mediane_seq / mediane_par` : x1.46 veut dire « le parallèle a mis
  1.46 fois moins de temps ».

### `Demo::controle()` `src/main.rs:415` — la vérification

- Chaque démo compare sa sortie seq et sa sortie par (ou donne une valeur
  de référence : `c[100] = 300`, `3.141593`, aller-retour de compression).
- **Un contrôle `false` = bug de parallélisme.** Ici, toujours `true`.

## 12. L'interface TUI

### `lancer()` `src/main.rs:464` — le worker

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

### `maj_ecran()` `src/main.rs:520` — l'affichage

```rust
Etape::Avance { texte, fait, total } => {
    siv.call_on_name("etape", |v: &mut TextView| v.set_content(texte));
    siv.call_on_name("barre", |v: &mut ProgressBar| v.set_value((fait*100)/total));
}
```

- `call_on_name("barre", ...)` retrouve la vue par son nom et la modifie.

### `main()` `src/main.rs:636` — la boucle

```rust
siv.set_fps(30);   // 30 images/s : la barre bouge même sans frappe
siv.add_global_callback(Event::Refresh, move |siv| { /* vide le canal */ });
```

- Une **seule** callback globale écoute `Event::Refresh` (toutes les 1/30 s) :
  elle vide le canal `mpsc` et met la barre à jour.
- Le `Mutex` autour du `Receiver` est obligatoire : cursive exige des
  callbacks `Send + Sync`, et un `Receiver` seul n'est que `Send`.

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