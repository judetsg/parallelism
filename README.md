# Mini-calculateur parallèle — Rust + rayon (expliqué ligne par ligne)

Programme console débutant : 5 petits calculs, chacun en version
séquentielle (1 thread) puis parallèle (N threads avec rayon),
avec chrono pour prouver le gain.

## 1. Installer et lancer

```bash
rustc --version   # doit afficher une version, sinon installer via https://rustup.rs/
cargo --version
cd parallelism
cargo run            # pour jouer
cargo run --release  # pour des temps réalistes (le tri est ~30x plus lent en debug)
```

`rayon = "1"` dans `Cargo.toml` se télécharge seul au premier run.

Menu :
```
1. Addition vecteurs (seq vs par)
2. Produit scalaire (seq vs par)
3. Pi (seq vs par)
4. Changer nb threads
5. Tri 1M valeurs (seq vs par)
6. Compression 64 Mo (seq vs par)
0. Quitter
```

Mesuré sur 8 threads, en `--release` (les chiffres varient selon la charge de la
machine) : Addition ~17ms→~7ms, Scalaire ~5ms→~3ms, Pi ~195ms→~81ms,
Tri ~120ms→~33ms, Compression ~325ms→~75ms.

## 2. Fichiers

```
Cargo.toml  -> rayon = "1", rien d'autre
src/main.rs -> tout le programme, ~320 lignes commentées
```

## 3. Imports `src/main.rs:5-8`, ligne par ligne

```rust
use rayon::prelude::*;      // :5 import .par_iter(), .into_par_iter(), .sum(), .for_each(), .par_sort_unstable()
use rayon::ThreadPoolBuilder; // :6 pour creer un pool a N threads (menu option 4)
use std::io::{self, Write};   // :7 lire clavier + flush pour afficher "> " aussitot
use std::time::Instant;       // :8 chrono seq vs par
```

Sans `:5`, `par_iter` n'existe pas. Sans `:6`, impossible de fixer N.
Sans `:7`, le `> ` ne s'affiche pas avant la frappe. Sans `:8`, pas de mesure.

## 4. `nb_threads()` `src/main.rs:10`

```rust
std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
```

- Demande au système le nb de coeurs logiques (ex: 8).
- `.get()` convertit en `usize`.
- `.unwrap_or(4)` : secours si le système ne répond pas.
- Appelé une fois dans `main()` `src/main.rs:276` pour la valeur initiale.

## 5. Addition vecteurs

### `vector_add_seq()` `src/main.rs:17` — référence 1 thread

```rust
for i in 0..a.len() { c[i] = a[i] + b[i]; }
```

- `a: &[f64]` = vue lecture, `c: &mut [f64]` = vue écriture.
- `i` va de 0 à n-1, on fait `c[i] = a[i]+b[i]`, ex: `c[100]=100+200=300`.
- Sert uniquement de temps de référence.

### `vector_add_par()` `src/main.rs:25` — version parallèle

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

### `dot_product_seq()` `src/main.rs:37`

```rust
let mut sum = 0.0;
for i in 0..a.len() { sum += a[i] * b[i]; }
sum
```

- Un seul `sum`, un seul thread, aucun conflit.

### `dot_product_par()` `src/main.rs:47` — somme parallèle

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

### `pi_seq()` `src/main.rs:55`

```rust
let step = 1.0 / steps as f64;   // largeur, ex: 1/20M
for i in 0..steps {
  let x = (i as f64 + 0.5) * step; // milieu du rectangle i
  sum += 4.0 / (1.0 + x * x);      // hauteur au milieu
}
sum * step                          // aire = somme hauteurs × largeur
```

### `pi_par()` `src/main.rs:67`

```rust
let step = 1.0 / steps as f64;  // partagé en lecture seule, pas de conflit
(0..steps).into_par_iter()      // range parallèle : rayon distribue les i
  .map(|i| { let x = (i as f64+0.5)*step; 4.0/(1.0+x*x) }) // hauteur par thread
  .sum::<f64>() * step          // reduction puis × largeur
```

- Même formule que seq, seul le partage des `i` change.
- 20M d'itérations : le plus lourd, speedup le plus visible.

## 8. Trier un gros tableau

Cas de la vie réelle : trier une colonne de 1 M de valeurs, comme quand on
trie 1 M de lignes dans un tableur.

### `hashe()` `src/main.rs:152` + `genere_valeurs()` `src/main.rs:162`

```rust
fn hashe(mut x: u64) -> u64 {        // splitmix64 : mélange reproductible
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

- `hashe(i)` : même index → même valeur, donc **les données sont identiques à chaque run**.
- Ça évite d'ajouter le crate `rand` : `Cargo.toml` reste à `rayon` tout seul.
- Le tableau est dans le désordre avant tri, comme une vraie colonne de tableur.

### `tri_seq()` `src/main.rs:169` — référence 1 thread

```rust
v.sort_unstable();
```

- Trie en place, un seul thread, tout le tableau d'un coup.

### `tri_par()` `src/main.rs:174` — version parallèle

```rust
v.par_sort_unstable();
```

- **Un seul appel** : rayon découpe la slice, trie chaque morceau en parallèle,
  puis fusionne les morceaux déjà triés entre eux (divide & merge).
- Ici le parallélisme est "gratuit" : pas de boucle à écrire, pas de bloc de
  Résultat à recoller, rayon gère le découpage et la fusion.
- Le tri est le meilleur exemple pour montrer qu'on n'a pas toujours à
  gérer les threads soi-même.

Contrôle dans `demo_tri()` `src/main.rs:180` : `trie ? true` (ordre croissant)
et `identique ? true` (les deux tris donnent le même tableau).

## 9. Compresser un gros fichier

Cas de la vie réelle : « zipe le fichier avant de l'envoyer ». Un fichier de
64 Mo qu'on découpe en 64 blocs de 1 Mo, chaque bloc compressé indépendamment.

### `genere_fichier()` `src/main.rs:201` — le faux fichier source

```rust
while i < n {
  let h = hashe(i as u64);
  let longueur = (h % 40) as usize + 1;   // suite de 1 à 40 octets identiques
  let fin = (i + longueur).min(n);
  v.resize(fin, (h >> 8) as u8);          // remplit de i à fin avec le même octet
  i = fin;
}
```

- On fabrique 64 Mo de données **compressibles** (des répétitions), comme un
  texte, un log ou une image.
- `resize(..., octet)` remplit d'une seule traite les octets manquants.

### `compresse_rle()` `src/main.rs:215` — la compression d'UN bloc

```rust
while i < bloc.len() {
  let v = bloc[i];                    // 1er octet de la suite
  let mut k = 1;                      // longueur (v compte déjà)
  while i + k < bloc.len() && bloc[i + k] == v && k < 255 { k += 1; }
  out.push(k as u8);                  // paire (longueur, valeur)
  out.push(v);
  i += k;                             // saute toute la suite
}
```

- RLE : chaque suite d'octets identiques devient 2 octets au lieu de `k`.
- Un bloc = une suite d'octets **sans aucun lien avec les autres blocs** :
  c'est exactement ce qui rend la compression parallélisable.
- 255 max par paire (1 octet pour la longueur).

### `compresse_seq()` `src/main.rs:244` / `compresse_par()` `src/main.rs:249`

```rust
blocs.iter().map(|b| compresse_rle(b)).collect()          // seq : 1 bloc après l'autre
blocs.par_iter().map(|b| compresse_rle(b)).collect()     // par : 64 blocs en même temps
```

- Même découpage `chunks(taille_bloc)` dans les deux cas → **octets identiques**.
- `.collect()` garde l'ordre des blocs : rayon ne mélange rien.

Contrôle dans `demo_compression()` `src/main.rs:254` :
`64 Mo -> 6 Mo (x10.3 plus petit)`, `identique ? true`,
`fichier reconstitue intact ? true` (aller-retour avec `decompresse()`).

## 10. Outils console

### `lire()` `src/main.rs:81`

```rust
let mut s = String::new();          // buffer vide
print!("> ");                       // invite
io::stdout().flush().unwrap();      // force l'affichage immédiat
io::stdin().read_line(&mut s).unwrap(); // bloque jusqu'à Entrée
s.trim().to_string()                // nettoie "\n"/espaces, ex: "1"
```

### `avec_pool()` `src/main.rs:90` — fixer le nombre de threads

```rust
ThreadPoolBuilder::new().num_threads(n_threads).build().unwrap().install(f)
```

- Crée un pool de exactement N travailleurs.
- `install(f)` y exécute `f` et attend la fin.
- On recrée le pool à chaque démo car le pool global rayon est fixe
  au démarrage et ne permet pas de changer N depuis le menu.
- Fonctionne aussi bien pour `f` qui renvoie `()` que pour `f` qui renvoie
  une valeur (`usize`, `Vec<Vec<u8>>`, ...), via le générique `<T: Send>`.

## 11. Démos `src/main.rs:99,119,135,180,254` — toutes pareilles

Pattern identique, exemple addition `src/main.rs:99` :

```rust
let a: Vec<f64> = (0..n).map(|i| i as f64).collect(); // a[i]=i
let b: Vec<f64> = (0..n).map(|i| i as f64 * 2.0).collect(); // b[i]=2*i
let t0 = Instant::now(); vector_add_seq(&a,&b,&mut c1); let d0 = t0.elapsed();
let t1 = Instant::now(); avec_pool(n, || vector_add_par(&a,&b,&mut c2)); let d1 = t1.elapsed();
println!("c[100] = {} (attendu 300)", c2[100]); // contrôle exact
println!("identique ? {}", c1 == c2);           // seq et par doivent matcher
```

- Scalaire attend `2000000` (= 1M × 1 × 2). Pi attend `3.141593`.
- Tri attend `trie ? true` + `identique ? true`.
- Compression attend `identique ? true` + `fichier reconstitue intact ? true`.
- Si `c1 != c2` ou Pi faux → bug de parallélisme (ici jamais).

## 12. `main()` `src/main.rs:276` — boucle de menu

```rust
let mut n_threads = nb_threads();  // ex: 8 au démarrage
loop {                              // infini jusqu'à "0"
  // affiche 1,2,3,4,5,6,0
  match lire().as_str() {           // lit le clavier
    "1" => demo_addition(n_threads),// lance la démo avec N actuel
    "2" => demo_dot(n_threads),
    "3" => demo_pi(n_threads),
    "5" => demo_tri(n_threads),
    "6" => demo_compression(n_threads),
    "4" => { lire().parse::<usize>() // convertit texte→nombre, accepte 1-64
             n_threads = v; }        // mémorise le nouveau N
    "0" => { println!("A bientot !"); break; } // sort du loop = fin
    _ => println!("Tape 0-6."),     // entrée invalide, on reboucle
  }
}
```

## 13. Comment ça fonctionne (à réciter)

- `par_iter` découpe les données en morceaux, un par thread du pool.
- Chaque thread calcule sa partie avec ses variables locales.
- `.sum()` additionne les résultats partiels à la fin.
- La fin de `for_each` / `sum` / `install` attend tous les threads.
- Pour un **tri**, rayon fait tout : il découpe, trie, puis fusionne.
- Pour un **gros fichier**, c'est nous qui le découpons en blocs, parce que
  rayon ne peut pas inventer un découpage qui a du sens pour le problème.

## 14. Questions pièges

- **Conflit d'écriture où ?** Dans `dot` / `pi` si `sum` était partagé.
  Évitée par sommes locales + `.sum()`.
- **Découpage et attente où ?** Découpage par `par_iter` / pool,
  attente à la fin de l'appel.
- **Pourquoi `par_sort_unstable` marche sans rien écrire ?** Le tri parallèle
  n'est pas un simple `map` : chaque morceau est trié indépendamment, puis
  les morceaux triés sont fusionnés. C'est rayon qui fait le découpage.
- **Pourquoi la compression marche ?** Parce qu'aucun bloc ne dépend d'un autre.
  Si le fichier était compressé d'un seul tenant (un seul flux), le découpage
  serait impossible : chaque bloc doit être compressible tout seul.
- **Pourquoi les sorties seq et par sont-elles identiques ?** Le découpage est
  fixé à l'avance (`chunks`) ou les données sont déterministes (`hashe(i)`),
  et l'ordre des résultats est conservé par `.collect()`.
- **Parfois par plus lent ?** Petit N : coût threads > gain. Trop de threads :
  plus de threads que de coeurs. Teste option 4 avec 1, 2, 4, 8.
- **En debug ?** Le tri prend ~2,5 s au lieu de ~90 ms : les comparaisons
  non optimisées écrasent tout. Mesure toujours avec `cargo run --release`.