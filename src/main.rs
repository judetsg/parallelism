// Mini-calculateur parallele : Rust + rayon
// But : 5 calculs simples, chacun en version _seq (1 thread) et _par (N threads).
// Lecture : chaque ligne de code a son commentaire apres //.

use rayon::prelude::*; // importe par_iter, into_par_iter, sum, for_each de rayon
use rayon::ThreadPoolBuilder; // importe le constructeur de pool a N threads
use std::io::{self, Write}; // importe lecture clavier (io) + flush d'affichage (Write)
use std::time::Instant; // importe le chrono pour mesurer seq vs par

fn nb_threads() -> usize {
    // demande au systeme le nb de coeurs logiques (ex: 8)
    std::thread::available_parallelism()
        .map(|n| n.get()) // convertit NonZero en usize
        .unwrap_or(4) // secours = 4 si le systeme ne repond pas
} // fin nb_threads

fn vector_add_seq(a: &[f64], b: &[f64], c: &mut [f64]) {
    // a = tableau lu no1, b = tableau lu no2, c = tableau ecrit
    for i in 0..a.len() {
        // i = index courant de 0 a n-1
        c[i] = a[i] + b[i]; // addition case par case : c[i] recoit a[i]+b[i]
    } // fin boucle seq, 1 seul thread a tout fait
} // fin vector_add_seq

fn vector_add_par(a: &[f64], b: &[f64], c: &mut [f64]) {
    // meme signature que seq, mais calcul partage entre threads
    a.par_iter() // iterateur parallele sur a, rayon va le decouper en chunks
        .zip(b.par_iter()) // associe chaque a[i] a son b[i], paire (a[i], b[i])
        .zip(c.par_iter_mut()) // associe en plus c[i] en ecriture
        .for_each(|((&x, &y), z)| {
            // closure executee en parallele sur chaque element
            // &x = copie de a[i], &y = copie de b[i], z = acces ecriture vers c[i]
            *z = x + y; // ecrit x+y dans c[i] via le pointeur z
        }); // fin for_each = barriere join, on attend tous les threads
} // fin vector_add_par

fn dot_product_seq(a: &[f64], b: &[f64]) -> f64 {
    // rend la somme a[0]*b[0] + a[1]*b[1] + ...
    let mut sum = 0.0; // accumulateur unique, pas de conflit car 1 thread
    for i in 0..a.len() {
        // parcourt tous les index
        sum += a[i] * b[i]; // ajoute le produit courant au total
    } // fin boucle
    sum // renvoie le total a l'appelant
} // fin dot_product_seq

fn dot_product_par(a: &[f64], b: &[f64]) -> f64 {
    // version parallele avec reduction (somme locale + somme finale)
    a.par_iter() // decoupe a entre les threads
        .zip(b.par_iter()) // paires (a[i], b[i])
        .map(|(&x, &y)| x * y) // chaque thread calcule son produit x*y
        .sum() // reduction finale : additionne les produits partiels
} // fin dot_product_par, pas de conflit d'ecriture grace a sum()

fn pi_seq(steps: u64) -> f64 {
    // approxime Pi par rectangles, steps = nb de rectangles
    let step = 1.0 / steps as f64; // largeur d'un rectangle = 1/steps
    let mut sum = 0.0; // somme des hauteurs des rectangles
    for i in 0..steps {
        // i = numero du rectangle courant
        let x = (i as f64 + 0.5) * step; // abscisse du milieu du rectangle
        sum += 4.0 / (1.0 + x * x); // hauteur = 4/(1+x^2), ajoutee au total
    } // fin boucle seq
    sum * step // aire totale = somme hauteurs * largeur, approx Pi
} // fin pi_seq

fn pi_par(steps: u64) -> f64 {
    // meme formule que pi_seq mais i distribues entre threads
    let step = 1.0 / steps as f64; // largeur partagee en lecture seule
    (0..steps) // intervalle de tous les numeros de rectangles
        .into_par_iter() // version parallele : rayon distribue les i par paquets
        .map(|i| {
            // chaque thread traite ses propres i
            let x = (i as f64 + 0.5) * step; // milieu du rectangle i
            4.0 / (1.0 + x * x) // renvoie la hauteur, sans toucher a sum partage
        }) // fin map parallele
        .sum::<f64>() // reduction : additionne toutes les hauteurs partielles
        * step // multiplie par la largeur comme en seq
} // fin pi_par

fn lire() -> String {
    // lit une ligne au clavier et la nettoie
    let mut s = String::new(); // buffer vide qui va recevoir la frappe
    print!("> "); // affiche l'invite
    io::stdout().flush().unwrap(); // force l'affichage immediat du >
    io::stdin().read_line(&mut s).unwrap(); // bloque jusqu'a Entree, remplit s
    s.trim().to_string() // enleve \n/espaces et renvoie copie propre ex "1"
} // fin lire

fn avec_pool<T: Send>(n_threads: usize, f: impl FnOnce() -> T + Send) -> T {
    // execute f dans un pool de exactement n_threads
    let pool = ThreadPoolBuilder::new() // prepare un nouveau pool
        .num_threads(n_threads) // fixe N travailleurs ex 8
        .build() // construit vraiment le pool
        .unwrap(); // echec si N invalide, ici N deja verifie 1-64
    pool.install(f) // lance f dedans, attend la fin join, renvoie son resultat
} // fin avec_pool

fn demo_addition(n_threads: usize) {
    // compare seq vs par sur addition 1M
    let n = 1_000_000usize; // 1M d'entiers = 4 Mo, seq ~2,5 s en debug et ~90 ms en release
    println!("\n--- Addition de {n} elements ({n_threads} threads, rayon) ---"); // titre demo
    let a: Vec<f64> = (0..n).map(|i| i as f64).collect(); // remplit a[i]=i ex a[100]=100
    let b: Vec<f64> = (0..n).map(|i| i as f64 * 2.0).collect(); // remplit b[i]=2*i ex 200
    let mut c1 = vec![0.0; n]; // resultat seq, zeros au depart
    let mut c2 = vec![0.0; n]; // resultat par, zeros au depart
    let t0 = Instant::now(); // demarre chrono seq
    vector_add_seq(&a, &b, &mut c1); // appelle seq : prete a,b et c1 modifiable
    let d0 = t0.elapsed(); // duree seq ex 52ms
    let t1 = Instant::now(); // demarre chrono par
    avec_pool(n_threads, || vector_add_par(&a, &b, &mut c2)); // appelle par dans pool N
    let d1 = t1.elapsed(); // duree par ex 37ms
    println!("seq : {d0:?}"); // affiche temps seq
    println!("par (rayon) : {d1:?}"); // affiche temps par
    println!("c[100] = {} (attendu 300)", c2[100]); // controle : 100+200=300
    println!("identique ? {}", c1 == c2); // vrai si seq et par donnent pareil
} // fin demo_addition

fn demo_dot(n_threads: usize) {
    // compare seq vs par sur produit scalaire 1M
    let n = 1_000_000usize; // 1M d'entiers = 4 Mo, seq ~2,5 s en debug et ~90 ms en release
    println!("\n--- Produit scalaire ({n_threads} threads, rayon) ---"); // titre
    let a = vec![1.0f64; n]; // 1M fois 1.0
    let b = vec![2.0f64; n]; // 1M fois 2.0
    let t0 = Instant::now(); // chrono seq
    let r0 = dot_product_seq(&a, &b); // seq attendu 1M*1*2=2000000
    let d0 = t0.elapsed(); // temps seq
    let t1 = Instant::now(); // chrono par
    let r1 = avec_pool(n_threads, || dot_product_par(&a, &b)); // par dans pool N
    let d1 = t1.elapsed(); // temps par
    println!("seq : {r0} en {d0:?}"); // affiche resultat+temps seq
    println!("par (rayon) : {r1} en {d1:?}"); // affiche resultat+temps par
} // fin demo_dot

fn demo_pi(n_threads: usize) {
    // compare seq vs par sur Pi 20M, le plus lourd donc speedup net
    let steps = 20_000_000u64; // 20M pas ~1s seq ~0.6s par sur 8 coeurs
    println!("\n--- Pi avec {steps} pas ({n_threads} threads, rayon) ---"); // titre
    let t0 = Instant::now(); // chrono seq
    let p0 = pi_seq(steps); // Pi seq ex 3.141593
    let d0 = t0.elapsed(); // temps seq
    let t1 = Instant::now(); // chrono par
    let p1 = avec_pool(n_threads, || pi_par(steps)); // Pi par dans pool N
    let d1 = t1.elapsed(); // temps par
    println!("seq : {p0:.6} en {d0:?}"); // :.6 = 6 decimales
    println!("par (rayon) : {p1:.6} en {d1:?}"); // meme format cote par
    println!("(vrai Pi = 3.141593)"); // reference de verification
} // fin demo_pi

// ================= Exemple 4 : trier un gros tableau =================

fn hashe(mut x: u64) -> u64 {
    // splitmix64 : transforme un entier en un autre, toujours le meme partout
    // (permet de fabriquer des donnees sans ajouter le crate rand)
    // Rappel : u64 = entier 64 bits, ^ = "ou exclusif", >> = decaler vers la droite
    // wrapping_add / wrapping_mul = idem + et *, mais "bouclent" au lieu de paniquer
    // quand le resultat depasse la taille de l'entier (c'est voulu ici)
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15); // constante standard de splitmix64
    let mut z = x; // copie de travail
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9); // premier melange
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB); // deuxieme melange
    z ^ (z >> 31) // troisieme melange = le resultat
} // fin hashe

fn genere_valeurs(n: usize) -> Vec<u32> {
    // fabrique n nombres entiers dans le desordre, toujours les memes d'une fois sur l'autre
    (0..n) // intervalle 0, 1, 2 ... n-1
        .map(|i| (hashe(i as u64) >> 32) as u32) // les 32 bits hauts du hash = un entier au hasard
        .collect() // renvoie le Vec<u32>
} // fin genere_valeurs

fn tri_seq(v: &mut [u32]) {
    // trie les valeurs en place, un seul thread
    v.sort_unstable(); // methode sequentielle de la bibliotheque standard
    // "unstable" = l'ordre des valeurs egales n'est pas garanti (sans importance ici)
} // fin tri_seq

fn tri_par(v: &mut [u32]) {
    // meme tri en parallele : rayon decoupe la slice en morceaux,
    // trie chaque morceau, puis fusionne les morceaux deja tries entre eux
    v.par_sort_unstable(); // un seul appel, rayon fait tout le travail
} // fin tri_par

fn demo_tri(n_threads: usize) {
    // compare seq vs par sur un tri de 1M valeurs, le cas ou rayon fait tout tout seul
    let n = 1_000_000usize; // 1M d'entiers = 4 Mo, seq ~2,5 s en debug et ~90 ms en release
    println!("\n--- Tri de {n} valeurs ({n_threads} threads, rayon) ---"); // titre demo
    let base = genere_valeurs(n); // donnees de depart, identiques pour les deux tris
    let mut v1 = base.clone(); // copie reservee au tri seq
    let mut v2 = base.clone(); // copie reservee au tri par
    let t0 = Instant::now(); // chrono seq
    tri_seq(&mut v1); // tri sequentiel, un seul thread
    let d0 = t0.elapsed(); // temps seq
    let t1 = Instant::now(); // chrono par
    avec_pool(n_threads, || tri_par(&mut v2)); // tri dans pool N
    let d1 = t1.elapsed(); // temps par
    println!("seq : {d0:?}"); // affiche temps seq
    println!("par (rayon) : {d1:?}"); // affiche temps par
    // windows(2) = toutes les paires voisines, all() = true si chaque paire est dans l'ordre
    println!("trie ? {}", v2.windows(2).all(|p| p[0] <= p[1])); // controle : valeurs en ordre croissant
    println!("identique ? {}", v1 == v2); // vrai si les deux tris donnent le meme tableau
} // fin demo_tri

// ================= Exemple 5 : compresser un gros fichier =================

fn genere_fichier(n: usize) -> Vec<u8> {
    // fabrique n octets qui se repetent, comme un texte, un log ou une image
    // Vec<u8> = tableau d'octets, c'est ce qu'est un fichier pour l'ordinateur
    let mut v = Vec::with_capacity(n); // reserve la place du fichier
    let mut i = 0; // position d'ecriture dans v
    while i < n { // tant qu'il reste des octets a ecrire
        let h = hashe(i as u64); // hasard reproductible -> pseudo-aleatoire
        let longueur = (h % 40) as usize + 1; // suite de 1 a 40 octets identiques
        let fin = (i + longueur).min(n); // on s'arrete exactement a n, min = le plus petit des deux
        v.resize(fin, (h >> 8) as u8); // remplit de i a fin avec le meme octet
        i = fin; // on passe a la suite suivante
    } // fin de la fabrication
    v // renvoie le faux fichier source
} // fin genere_fichier

fn compresse_rle(bloc: &[u8]) -> Vec<u8> {
    // RLE : chaque suite d'octets identiques devient une paire (longueur, valeur)
    let mut out = Vec::new(); // bloc compresse, vide au depart
    let mut i = 0; // position dans bloc
    while i < bloc.len() { // tant qu'il reste des octets
        let v = bloc[i]; // premier octet de la suite
        let mut k = 1; // longueur de la suite, v compte deja
        while i + k < bloc.len() && bloc[i + k] == v && k < 255 { // on compte les repetitions, 255 max
            k += 1; // une repetition de plus
        } // fin du comptage
        out.push(k as u8); // ecrit la longueur sur un octet
        out.push(v); // puis la valeur repetee
        i += k; // saute toute la suite d'un coup
    } // fin du bloc
    out // renvoie le bloc compresse
} // fin compresse_rle

fn decompresse(compresse: &[Vec<u8>]) -> Vec<u8> {
    // operation inverse : relit les paires (longueur, valeur) et re-ecrit les octets
    let mut out = Vec::new(); // fichier reconstitue
    for bloc in compresse { // on parcourt les blocs dans l'ordre
        for paire in bloc.chunks_exact(2) { // chunks_exact(2) = decoupe le bloc par groupes de 2 octets
            let (k, v) = (paire[0] as usize, paire[1]); // longueur et valeur de la suite
            out.resize(out.len() + k, v); // ajoute k fois v a la fin
        } // fin des paires du bloc
    } // fin des blocs
    out // renvoie le fichier d'origine
} // fin decompresse

fn compresse_seq(blocs: &[&[u8]]) -> Vec<Vec<u8>> {
    // compresse les blocs l'un apres l'autre, un seul thread
    // Vec<Vec<u8>> = une liste de blocs compresses, chaque bloc = un Vec d'octets
    blocs.iter().map(|b| compresse_rle(b)).collect() // un bloc compresse par bloc d'origine, meme ordre
} // fin compresse_seq

fn compresse_par(blocs: &[&[u8]]) -> Vec<Vec<u8>> {
    // meme chose, mais chaque bloc part sur un thread different
    blocs.par_iter().map(|b| compresse_rle(b)).collect() // rayon garde l'ordre des blocs
} // fin compresse_par

fn demo_compression(n_threads: usize) {
    // compare seq vs par sur la compression d'un fichier de 64 Mo coupe en 64 blocs
    let taille = 64 * 1024 * 1024; // 64 Mo, la taille d'un vrai fichier a envoyer
    let n_blocs = 64; // 64 blocs independants = 64 taches a distribuer
    let taille_bloc = taille / n_blocs; // 1 Mo par bloc
    println!("\n--- Compression de {} Mo en {n_blocs} blocs ({n_threads} threads, rayon) ---", taille / 1024 / 1024); // titre demo, /1024/1024 = conversion en Mo
    let fichier = genere_fichier(taille); // faux fichier source, fabrique sur place
    let blocs: Vec<&[u8]> = fichier.chunks(taille_bloc).collect(); // chunks = decoupe en morceaux de taille_bloc
    let t0 = Instant::now(); // chrono seq
    let c0 = compresse_seq(&blocs); // compression sequentielle
    let d0 = t0.elapsed(); // temps seq
    let t1 = Instant::now(); // chrono par
    let c1 = avec_pool(n_threads, || compresse_par(&blocs)); // compression parallele dans pool N
    let d1 = t1.elapsed(); // temps par
    // sum() additionne la taille de chaque bloc compresse = taille totale du fichier compresse
    let taille_finale: usize = c1.iter().map(|b| b.len()).sum();
    println!("seq : {d0:?}"); // affiche temps seq
    println!("par (rayon) : {d1:?}"); // affiche temps par
    println!("{} Mo -> {} Mo (x{:.1} plus petit)", taille / 1024 / 1024, taille_finale / 1024 / 1024, taille as f64 / taille_finale as f64); // gain de place, :.1 = 1 decimale
    println!("identique ? {}", c0 == c1); // vrai si seq et par donnent les memes octets
    println!("fichier reconstitue intact ? {}", decompresse(&c1) == fichier); // aller-retour : on retrouve l'original
} // fin demo_compression

fn main() {
    // boucle de menu, point d'entree du programme
    let mut n_threads = nb_threads(); // initialise N avec nb coeurs ex 8
    println!("Mini-calculateur parallele (Rust + rayon)"); // titre programme
    println!("Threads detectes : {n_threads}"); // rappelle N initial
    loop {
        // boucle infinie, sortie seulement par break sur "0"
        println!("\n=== MENU ==="); // separateur visuel
        println!("1. Addition vecteurs (seq vs par)"); // choix 1
        println!("2. Produit scalaire (seq vs par)"); // choix 2
        println!("3. Pi (seq vs par)"); // choix 3
        println!("4. Changer nb threads (actuel : {n_threads})"); // choix 4 avec N courant
        println!("5. Tri 1M valeurs (seq vs par)"); // choix 5
        println!("6. Compression 64 Mo (seq vs par)"); // choix 6
        println!("0. Quitter"); // choix 0
        match lire().as_str() {
            // lit clavier puis compare le texte
            "1" => demo_addition(n_threads), // tape 1 lance demo addition avec N
            "2" => demo_dot(n_threads), // tape 2 lance demo scalaire
            "3" => demo_pi(n_threads), // tape 3 lance demo Pi
            "4" => {
                // tape 4 veut changer N
                println!("Nouveau nombre (1-64) :"); // demande la valeur
                match lire().parse::<usize>() {
                    // convertit texte en nombre entier
                    Ok(v) if v >= 1 && v <= 64 => {
                        // garde-fou : accepte 1 a 64 seulement
                        n_threads = v; // memorise le nouveau N
                        println!("OK, threads = {n_threads}"); // confirme
                    } // fin cas valide
                    _ => println!("Invalide."), // cas texte non-nombre ou hors borne
                } // fin match conversion
            } // fin choix 4
            "5" => demo_tri(n_threads), // tape 5 lance demo tri avec N
            "6" => demo_compression(n_threads), // tape 6 lance demo compression avec N
            "0" => {
                // tape 0 veut quitter
                println!("A bientot !"); // message au revoir
                break; // sort du loop, fin du programme
            } // fin choix 0
            _ => println!("Tape 0-6."), // tout autre texte = erreur douce, on reboucle
        } // fin match menu
    } // fin loop
} // fin main
