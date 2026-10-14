# Mini-calculateur parallèle — Rust + rayon (expliqué ligne par ligne)

Programme console débutant : 3 petits calculs, chacun en version
séquentielle (1 thread) puis parallèle (N threads avec rayon),
avec chrono pour prouver le gain.

## 1. Installer et lancer

```bash
rustc --version   # doit afficher une version, sinon installer via https://rustup.rs/
cargo --version
cd parallelism
cargo run
```

`rayon = "1"` dans `Cargo.toml` se télécharge seul au premier run.

Menu :
```
1. Addition vecteurs (seq vs par)
2. Produit scalaire (seq vs par)
3. Pi (seq vs par)
4. Changer nb threads
0. Quitter
```

Mesuré sur 8 threads : Addition 52ms→37ms, Scalaire 54ms→34ms, Pi 1.00s→0.64s.

## 2. Fichiers

```
Cargo.toml  -> rayon = "1", rien d'autre
src/main.rs -> tout le programme, ~220 lignes commentées
```

## 3. Imports `src/main.rs:6-9`, ligne par ligne

```rust
use rayon::prelude::*;      // :6 import .par_iter(), .into_par_iter(), .sum(), .for_each()
use rayon::ThreadPoolBuilder; // :7 pour creer un pool a N threads (menu option 4)
use std::io::{self, Write};   // :8 lire clavier + flush pour afficher "> " aussitot
use std::time::Instant;       // :9 chrono seq vs par
```

Sans `:6`, `par_iter` n'existe pas. Sans `:7`, impossible de fixer N.
Sans `:8`, le `> ` ne s'affiche pas avant la frappe. Sans `:9`, pas de mesure.

## 4. `nb_threads()` `src/main.rs:15`

```rust
std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
```

- Demande au système le nb de coeurs logiques (ex: 8).
- `.get()` convertit en `usize`.
- `.unwrap_or(4)` : secours si le système ne répond pas.
- Appelé une fois dans `main()` `src/main.rs:184` pour la valeur initiale.

## 5. Addition vecteurs

### `vector_add_seq()` `src/main.rs:25` — référence 1 thread

```rust
for i in 0..a.len() { c[i] = a[i] + b[i]; }
```

- `a: &[f64]` = vue lecture, `c: &mut [f64]` = vue écriture.
- `i` va de 0 à n-1, on fait `c[i] = a[i]+b[i]`, ex: `c[100]=100+200=300`.
- Sert uniquement de temps de référence.

### `vector_add_par()` `src/main.rs:34` — version parallèle

```rust
a.par_iter()            // vue parallèle de a, rayon va la découper en chunks
  .zip(b.par_iter())    // colle b : paire (a[i], b[i]) pour chaque i
  .zip(c.par_iter_mut())// colle c en écriture : ((a[i],b[i]), c[i])
  .for_each(|((&x, &y), z)| { *z = x + y; });
```

- `&x` = copie de `a[i]`, `&y` = copie de `b[i]`, `z` = accès écriture à `c[i]`.
- `*z = ...` = écrire dans la case.
- Chaque `c[i]` est écrit une seule fois par un seul thread → pas de conflit.
- Fin du `for_each` = barrière : on attend tous les threads.

## 6. Produit scalaire (reduction)

### `dot_product_seq()` `src/main.rs:48`

```rust
let mut sum = 0.0;
for i in 0..a.len() { sum += a[i] * b[i]; }
sum
```

- Un seul `sum`, un seul thread, aucun conflit.

### `dot_product_par()` `src/main.rs:59` — somme parallèle

```rust
a.par_iter().zip(b.par_iter()).map(|(&x, &y)| x * y).sum()
```

- `.map(|(&x,&y)| x*y)` : chaque thread calcule ses produits sur son morceau.
- `.sum()` : rayon additionne les sommes partielles à la fin.
- Si on partageait `sum` directement entre threads, 2 threads écriraient
  en même temps et le résultat serait faux.
- La parade "somme locale + somme finale" évite ce conflit.

## 7. Pi par rectangles

Idée maths : aire sous `4/(1+x²)` entre 0 et 1 = Pi.
On coupe en `steps` rectangles de largeur `step = 1/steps`.

### `pi_seq()` `src/main.rs:70`

```rust
let step = 1.0 / steps as f64;   // largeur, ex: 1/20M
for i in 0..steps {
  let x = (i as f64 + 0.5) * step; // milieu du rectangle i
  sum += 4.0 / (1.0 + x * x);      // hauteur au milieu
}
sum * step                          // aire = somme hauteurs × largeur
```

### `pi_par()` `src/main.rs:83`

```rust
let step = 1.0 / steps as f64;  // partagé en lecture seule, pas de conflit
(0..steps).into_par_iter()      // range parallèle : rayon distribue les i
  .map(|i| { let x = (i as f64+0.5)*step; 4.0/(1.0+x*x) }) // hauteur par thread
  .sum::<f64>() * step          // reduction puis × largeur
```

- Même formule que seq, seul le partage des `i` change.
- 20M d'itérations : le plus lourd, speedup le plus visible.

## 8. Outils console

### `lire()` `src/main.rs:101`

```rust
let mut s = String::new();          // buffer vide
print!("> ");                       // invite
io::stdout().flush().unwrap();      // force l'affichage immédiat
io::stdin().read_line(&mut s).unwrap(); // bloque jusqu'à Entrée
s.trim().to_string()                // nettoie "\n"/espaces, ex: "1"
```

### `avec_pool()` `src/main.rs:113` — fixer le nombre de threads

```rust
ThreadPoolBuilder::new().num_threads(n_threads).build().unwrap().install(f)
```

- Crée un pool de exactement N travailleurs.
- `install(f)` y exécute `f` et attend la fin.
- On recrée le pool à chaque démo car le pool global rayon est fixe
  au démarrage et ne permet pas de changer N depuis le menu.

## 9. Démos `src/main.rs:124,147,166` — toutes pareilles

Pattern identique, exemple addition `src/main.rs:124` :

```rust
let a: Vec<f64> = (0..n).map(|i| i as f64).collect(); // a[i]=i
let b: Vec<f64> = (0..n).map(|i| i as f64 * 2.0).collect(); // b[i]=2*i
let t0 = Instant::now(); vector_add_seq(&a,&b,&mut c1); let d0 = t0.elapsed();
let t1 = Instant::now(); avec_pool(n, || vector_add_par(&a,&b,&mut c2)); let d1 = t1.elapsed();
println!("c[100] = {} (attendu 300)", c2[100]); // contrôle exact
println!("identique ? {}", c1 == c2);           // seq et par doivent matcher
```

- Scalaire attend `2000000` (= 1M × 1 × 2). Pi attend `3.141593`.
- Si `c1 != c2` ou Pi faux → bug de parallélisme (ici jamais).

## 10. `main()` `src/main.rs:184` — boucle de menu

```rust
let mut n_threads = nb_threads();  // ex: 8 au démarrage
loop {                              // infini jusqu'à "0"
  // affiche 1,2,3,4,0
  match lire().as_str() {           // lit le clavier
    "1" => demo_addition(n_threads),// lance la démo avec N actuel
    "2" => demo_dot(n_threads),
    "3" => demo_pi(n_threads),
    "4" => { lire().parse::<usize>() // convertit texte→nombre, accepte 1-64
             n_threads = v; }        // mémorise le nouveau N
    "0" => { println!("A bientot !"); break; } // sort du loop = fin
    _ => println!("Tape 0-4."),     // entrée invalide, on reboucle
  }
}
```

## 11. Comment ça fonctionne (à réciter)

- `par_iter` découpe les données en morceaux, un par thread du pool.
- Chaque thread calcule sa partie avec ses variables locales.
- `.sum()` additionne les résultats partiels à la fin.
- La fin de `for_each` / `sum` / `install` attend tous les threads.

## 12. Questions pièges

- **Conflit d'écriture où ?** Dans `dot` / `pi` si `sum` était partagé.
  Évitée par sommes locales + `.sum()`.
- **Découpage et attente où ?** Découpage par `par_iter` / pool,
  attente à la fin de l'appel.
- **Parfois par plus lent ?** Petit N : coût threads > gain. Trop de threads :
  plus de threads que de coeurs. Teste option 4 avec 1, 2, 4, 8.
- **`cargo run --release`** : mode optimisé, encore plus rapide.
