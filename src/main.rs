// Mini-calculateur parallele : Rust + rayon, pilote par une interface TUI
// But : 5 calculs simples, chacun en version _seq (1 thread) et _par (N threads),
//        mesures plusieurs fois puis affiche min / mediane / max et le gain.
// Lecture : chaque ligne de code a son commentaire apres //.

use cursive::event::{Event, Key}; // interface TUI : evenements clavier
use cursive::theme::Effect; // effets d'affichage : Bold = gras, Dim = gris
use cursive::view::{Nameable, Resizable}; // with_name() nomme une vue, fixed_* fixe sa taille
use cursive::views::{Button, EditView, LinearLayout, OnEventView, Panel, ProgressBar, ScrollView, SelectView, TextView}; // briques de l'interface
use cursive::Cursive; // l'application TUI elle-meme
use rayon::prelude::*; // importe par_iter, into_par_iter, sum, for_each, par_sort_unstable de rayon
use rayon::ThreadPoolBuilder; // importe le constructeur de pool a N threads
use std::sync::atomic::{AtomicBool, Ordering}; // drapeau d'annulation partage entre l'ecran et le worker
use std::sync::mpsc::{self, Sender}; // canal pour envoyer les news du calcul a l'interface
use std::sync::Mutex; // rend le canal partage entre l'interface et le worker
use std::time::{Duration, Instant}; // chrono pour mesurer seq vs par

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

fn avec_pool<T: Send>(n_threads: usize, f: impl FnOnce() -> T + Send) -> T {
    // execute f dans un pool de exactement n_threads
    let pool = ThreadPoolBuilder::new() // prepare un nouveau pool
        .num_threads(n_threads) // fixe N travailleurs ex 8
        .build() // construit vraiment le pool
        .unwrap(); // echec si N invalide, ici N toujours >= 1
    pool.install(f) // lance f dedans, attend la fin join, renvoie son resultat
} // fin avec_pool

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

// ================= Pilotage : une variante = une mesure de temps =================

// Chaque demonstration expose 2 variantes (seq et par) via run().
// Une variante prepare ses donnees PUIS chronometre uniquement le calcul :
// les repetitions ne sont donc pas fausseees par un tableau deja trie ou deja calcule.

fn run_addition(n_threads: usize, par: bool) -> (Duration, Vec<f64>) {
    // prepare 1M d'elements puis chronometre uniquement l'addition
    // Le calcul est repete PASSES fois : sinon on ne fait que lire 24 Mo de memoire,
    // et le goulot d'etanglement est la memoire, pas le CPU -> le par ne gagnerait rien.
    const PASSES: usize = 10; // repetitions du calcul dans la zone chronometree
    let n = 1_000_000usize; // 1M d'elements, ex a[100]=100 et b[100]=200
    let a: Vec<f64> = (0..n).map(|i| i as f64).collect(); // a[i]=i, hors chrono
    let b: Vec<f64> = (0..n).map(|i| i as f64 * 2.0).collect(); // b[i]=2*i, hors chrono
    let mut c = vec![0.0; n]; // resultat, zeros au depart, hors chrono
    let t0 = Instant::now(); // rien d'autre que le calcul entre t0 et elapsed
    if par {
        // variante rayon : chaque passage est partage entre n_threads threads
        avec_pool(n_threads, || {
            for _ in 0..PASSES {
                // on additionne PASSES fois, c'est le nombre de multiplications qui compte
                vector_add_par(&a, &b, &mut c);
            }
        });
    } else {
        // variante sequentielle : un seul thread
        for _ in 0..PASSES {
            // meme calcul, mais par un seul thread
            vector_add_seq(&a, &b, &mut c);
        }
    } // fin du calcul
    let d = t0.elapsed(); // on garde uniquement la duree du calcul
    (d, c) // duree + resultat, pour le controle final (c[100] = 300, a et b ne changent pas)
} // fin run_addition

fn run_dot(n_threads: usize, par: bool) -> (Duration, f64) {
    // prepare 1M de couples (1,0 ; 2,0) puis chronometre uniquement la somme
    let n = 1_000_000usize; // 1M d'elements
    let a = vec![1.0f64; n]; // 1M fois 1.0, hors chrono
    let b = vec![2.0f64; n]; // 1M fois 2.0, hors chrono
    let t0 = Instant::now(); // chrono start
    let r = if par {
        avec_pool(n_threads, || dot_product_par(&a, &b)) // version rayon
    } else {
        dot_product_seq(&a, &b) // version 1 thread
    };
    let d = t0.elapsed(); // chrono stop
    (d, r) // duree + resultat, attendu 2 000 000
} // fin run_dot

fn run_pi(n_threads: usize, par: bool) -> (Duration, f64) {
    // chronometre uniquement l'approximation de Pi, les donnees n'existent pas
    let steps = 20_000_000u64; // 20M de rectangles, le calcul le plus lourd
    let t0 = Instant::now(); // chrono start
    let p = if par {
        avec_pool(n_threads, || pi_par(steps)) // version rayon
    } else {
        pi_seq(steps) // version 1 thread
    };
    let d = t0.elapsed(); // chrono stop
    (d, p) // duree + Pi approxime, attendu 3.141593
} // fin run_pi

fn run_tri(n_threads: usize, par: bool) -> (Duration, Vec<u32>) {
    // prepare 1M d'entiers melanges puis chronometre uniquement le tri
    let n = 1_000_000usize; // 1M d'entiers = 4 Mo
    let mut v = genere_valeurs(n); // tableau dans le desordre, hors chrono
    // Important : on regenere les valeurs a chaque repetition, sinon on trierait
    // un tableau deja trie et la mesure serait fausse.
    let t0 = Instant::now(); // chrono start
    if par {
        avec_pool(n_threads, || tri_par(&mut v)); // version rayon
    } else {
        tri_seq(&mut v); // version 1 thread
    }
    let d = t0.elapsed(); // chrono stop
    (d, v) // duree + tableau trie, pour le controle final
} // fin run_tri

fn run_compression(n_threads: usize, par: bool) -> (Duration, Vec<Vec<u8>>) {
    // prepare 64 Mo de donnees compressibles puis chronometre uniquement la compression
    let taille = 64 * 1024 * 1024; // 64 Mo, la taille d'un vrai fichier a envoyer
    let n_blocs = 64; // 64 blocs independants = 64 taches a distribuer
    let taille_bloc = taille / n_blocs; // 1 Mo par bloc
    let fichier = genere_fichier(taille); // faux fichier source, hors chrono
    let blocs: Vec<&[u8]> = fichier.chunks(taille_bloc).collect(); // chunks = decoupe fixe en morceaux
    let t0 = Instant::now(); // chrono start
    let c = if par {
        avec_pool(n_threads, || compresse_par(&blocs)) // version rayon
    } else {
        compresse_seq(&blocs) // version 1 thread
    };
    let d = t0.elapsed(); // chrono stop
    (d, c) // duree + fichier compresse, pour le controle final
} // fin run_compression

// ================= Mesures : min / mediane / max et gain =================

static ANNULATION: AtomicBool = AtomicBool::new(false); // passe a true si l'utilisateur quitte l'ecran

struct Stats {
    min: Duration, // la plus rapide des repetitions
    mediane: Duration, // la repetition du milieu apres tri des durees
    max: Duration, // la plus lente des repetitions
} // fin Stats

fn duree(d: Duration) -> String {
    // formate une duree lisible : 850 us, 16.8 ms, 1.20 s
    let ms = d.as_secs_f64() * 1000.0; // tout converti en millisecondes
    if ms < 1.0 {
        format!("{:.0} us", ms * 1000.0) // tres court : microsecondes
    } else if ms < 1000.0 {
        format!("{:.1} ms", ms) // classique : millisecondes
    } else {
        format!("{:.2} s", ms / 1000.0) // long : secondes
    } // fin du choix d'unite
} // fin duree

fn mesurer(reps: usize, mut variante: impl FnMut() -> Duration) -> Option<Stats> {
    // repete la variante reps fois et garde min / mediane / max
    // None = l'utilisateur a quitte l'ecran, on jette la mesure incomplete
    let mut durees: Vec<Duration> = Vec::with_capacity(reps); // les durees des repetitions
    for _ in 0..reps { // on repete reps fois
        if ANNULATION.load(Ordering::Relaxed) {
            // l'ecran a ete quitte : on stoppe avant de lancer une repetition de plus
            return None;
        } // fin du test d'annulation
        durees.push(variante()); // chaque appel prepare ses donnees puis chronometre seul le calcul
    } // fin des repetitions
    durees.sort(); // Duration se compare avec <, on trie donc les durees
    Some(Stats {
        min: durees[0], // la 1ere apres tri
        mediane: durees[reps / 2], // la valeur du milieu (pour un nombre pair, celle de droite)
        max: durees[reps - 1], // la derniere apres tri
    }) // fin des statistiques
} // fin mesurer

fn formater(seq: Stats, par: Stats, controle: &str) -> String {
    // assemble le rapport affiche sous la barre de progression
    let gain = seq.mediane.as_secs_f64() / par.mediane.as_secs_f64(); // seq divise par par
    format!(
        // 1ere ligne : les 3 durees de la variante sequentielle
        " seq  : min {:>9}  mediane {:>9}  max {:>9}\n par  : min {:>9}  mediane {:>9}  max {:>9}\n gain : x{:.2} (seq / par, sur la mediane)\n controle : {}",
        duree(seq.min),
        duree(seq.mediane),
        duree(seq.max),
        duree(par.min),
        duree(par.mediane),
        duree(par.max),
        gain,
        controle
    ) // fin du rapport
} // fin formater

// ================= Les 5 demonstrations =================

enum Resultat {
    Reels(Vec<f64>), // tableau de doubles, ex: addition
    Nombre(f64), // un seul nombre, ex: scalaire et Pi
    Entiers(Vec<u32>), // tableau d'entiers, ex: tri
    Blocs(Vec<Vec<u8>>), // fichier compresse en blocs, ex: compression
} // fin Resultat

#[derive(Clone, Copy, PartialEq)]
enum Demo {
    Addition, // addition de deux vecteurs
    Scalaire, // produit scalaire (reduction)
    Pi, // approximation de Pi (reduction)
    Tri, // tri d'un gros tableau
    Compression, // compression d'un fichier par blocs
} // fin Demo

impl Demo {
    fn tous() -> [Demo; 5] {
        // les 5 demonstrations, dans l'ordre du menu
        [Demo::Addition, Demo::Scalaire, Demo::Pi, Demo::Tri, Demo::Compression]
    } // fin tous

    fn nom(&self) -> &'static str {
        // nom affiche dans le menu
        match self {
            Demo::Addition => "1. Addition de vecteurs (1 M)",
            Demo::Scalaire => "2. Produit scalaire (1 M)",
            Demo::Pi => "3. Pi par rectangles (20 M)",
            Demo::Tri => "4. Tri d'un tableau (1 M)",
            Demo::Compression => "5. Compression d'un fichier (64 Mo)",
        } // fin du nom
    } // fin nom

    fn detail(&self) -> &'static str {
        // petite phrase affichee pendant la mesure
        match self {
            Demo::Addition => "chaque case depend d'une seule case : map sur place",
            Demo::Scalaire => "une seule somme pour 1 M de produits : reduction",
            Demo::Pi => "20 M de rectangles independants : reduction sur un intervalle",
            Demo::Tri => "rayon decoupe, trie chaque morceau, puis fusionne",
            Demo::Compression => "64 blocs de 1 Mo, compresses separement puis recolles",
        } // fin du detail
    } // fin detail

    fn run(&self, n_threads: usize, par: bool) -> (Duration, Resultat) {
        // lance UNE repetition et renvoie sa duree + son resultat
        match self {
            Demo::Addition => {
                let (d, c) = run_addition(n_threads, par); // addition
                (d, Resultat::Reels(c)) // resultat = le vecteur calcule
            }
            Demo::Scalaire => {
                let (d, r) = run_dot(n_threads, par); // produit scalaire
                (d, Resultat::Nombre(r)) // resultat = la somme
            }
            Demo::Pi => {
                let (d, p) = run_pi(n_threads, par); // Pi
                (d, Resultat::Nombre(p)) // resultat = Pi approxime
            }
            Demo::Tri => {
                let (d, v) = run_tri(n_threads, par); // tri
                (d, Resultat::Entiers(v)) // resultat = le tableau trie
            }
            Demo::Compression => {
                let (d, c) = run_compression(n_threads, par); // compression
                (d, Resultat::Blocs(c)) // resultat = le fichier compresse
            }
        } // fin du choix de la demonstration
    } // fin run

    fn controle(&self, n_threads: usize) -> String {
        // verifie le resultat, une seule fois, hors mesure
        let (_, r0) = self.run(n_threads, false); // un passage seq pour reference
        let (_, r1) = self.run(n_threads, true); // le passage par
        match (r0, r1) {
            // Addition : on compare les 2 vecteurs case par case
            (Resultat::Reels(c0), Resultat::Reels(c1)) => {
                format!("c[100] = {} (attendu 300) | identique ? {}", c1[100], c0 == c1) // comparaison
            }
            // Scalaire ou Pi : reduction, l'ordre des additions peut changer les derniers bits
            (Resultat::Nombre(r0), Resultat::Nombre(r1)) => match *self {
                Demo::Pi => format!("seq = {r0:.6} | par = {r1:.6} (vrai Pi = 3.141593)"), // 6 decimales
                _ => format!("seq = {r0} | par = {r1} (attendu 2000000)"), // produit scalaire
            },
            // Tri : le tableau par doit etre trie, et identique a celui du seq
            (Resultat::Entiers(v0), Resultat::Entiers(v1)) => {
                // windows(2) = toutes les paires voisines, all() = true si chaque paire est dans l'ordre
                let trie = v1.windows(2).all(|p| p[0] <= p[1]);
                format!("trie ? {trie} | identique ? {}", v0 == v1) // comparaison des 2 tris
            }
            // Compression : memes octets, et l'aller-retour redonne le fichier d'origine
            (Resultat::Blocs(c0), Resultat::Blocs(c1)) => {
                let taille = 64 * 1024 * 1024; // meme taille que dans run_compression
                let fichier = genere_fichier(taille); // on regenere le fichier pour l'aller-retour
                // sum() additionne la taille de chaque bloc compresse = taille totale
                let taille_finale: usize = c1.iter().map(|b| b.len()).sum();
                format!(
                    "{} Mo -> {} Mo (x{:.1}) | identique ? {} | reconstitue intact ? {}",
                    taille / 1024 / 1024,
                    taille_finale / 1024 / 1024,
                    taille as f64 / taille_finale as f64,
                    c0 == c1,
                    decompresse(&c1) == fichier
                ) // fin du controle
            }
            // les 2 passages d'une meme demo rendent toujours le meme type : jamais atteint
            _ => "controle impossible".to_string(), // filet de securite
        } // fin du controle
    } // fin controle
} // fin impl Demo

// ================= Le worker : fait les mesures, l'ecran ecoute =================

enum Etape {
    Avance { texte: String, fait: usize, total: usize }, // une repetition vient de finir
    Termine { seq: Stats, par: Stats, controle: String }, // toutes les mesures sont pretes
    Annule, // l'utilisateur a quitte l'ecran avant la fin
} // fin Etape

fn lancer(demo: Demo, n_threads: usize, reps: usize, tx: Sender<Etape>) {
    // le calcul se fait ICI, dans un thread a part : l'interface reste fluide
    let total = reps * 2 + 1; // seq (reps) + par (reps) + 1 controle
    let mut fait = 0usize; // compteur d'avancement global (les 3 phases en font un total)
    let mut rep = 0usize; // compteur de repetitions dans la phase courante
    // --- phase 1 : la variante sequentielle ---
    let seq = match mesurer(reps, || {
        // une repetition sequentielle, donnees neuves a chaque fois
        let d = demo.run(n_threads, false).0; // on ne garde que la duree
        rep += 1; // repetition 1, 2, 3... de cette phase
        fait += 1; // une repetition de plus au total
        let _ = tx.send(Etape::Avance {
            texte: format!("sequentiel {rep}/{reps}"),
            fait,
            total,
        }); // on previent l'interface
        d // la duree mesuree
    }) {
        Some(stats) => stats, // toutes les repetitions sont faites
        None => {
            let _ = tx.send(Etape::Annule); // abandon : on previent l'interface
            return; // et le worker termine
        } // fin du cas abandon
    }; // fin de la phase 1
    rep = 0; // on repart a zero pour la phase parallele
    // --- phase 2 : la variante parallele ---
    let par = match mesurer(reps, || {
        // une repetition parallele, donnees neuves a chaque fois
        let d = demo.run(n_threads, true).0; // on ne garde que la duree
        rep += 1; // repetition 1, 2, 3... de cette phase
        fait += 1; // une repetition de plus au total
        let _ = tx.send(Etape::Avance {
            texte: format!("parallele {rep}/{reps}"),
            fait,
            total,
        }); // on previent l'interface
        d // la duree mesuree
    }) {
        Some(stats) => stats,
        None => {
            let _ = tx.send(Etape::Annule);
            return;
        } // fin du cas abandon
    }; // fin de la phase 2
    // --- phase 3 : le controle de correction ---
    let controle = demo.controle(n_threads); // verifie seq vs par, une fois, hors chrono
    fait += 1; // la barre arrive a fond
    let _ = tx.send(Etape::Avance {
        texte: "controle".to_string(),
        fait,
        total,
    });
    // --- fin : on envoie le rapport ---
    let _ = tx.send(Etape::Termine { seq, par, controle }); // l'ecran affiche le tableau
} // fin lancer

fn maj_ecran(siv: &mut Cursive, evenement: Etape) {
    // met l'ecran de mesure a jour, appele 30 fois par seconde
    match evenement {
        Etape::Avance { texte, fait, total } => {
            // une repetition vient de finir : on avance la barre et le texte
            siv.call_on_name("etape", |v: &mut TextView| v.set_content(texte)); // libelle
            siv.call_on_name("barre", |v: &mut ProgressBar| {
                v.set_value((fait * 100) / total); // la barre va de 0 a 100
                // set_label attend une fonction : elle recoit (valeur, max) et renvoie le texte
                v.set_label(move |pourcent, _| format!("{pourcent} %"));
            }); // barre
        } // fin de l'avancement
        Etape::Termine { seq, par, controle } => {
            // toutes les mesures sont pretes : on affiche le rapport
            siv.call_on_name("etape", |v: &mut TextView| v.set_content("Termine.")); // libelle
            siv.call_on_name("barre", |v: &mut ProgressBar| {
                v.set_value(100); // barre pleine
                v.set_label(|_, _| "OK".to_string()); // libelle final
            }); // barre
            siv.call_on_name("resultats", |v: &mut TextView| {
                v.set_content(formater(seq, par, &controle))
            }); // rapport complet
        } // fin du rapport
        Etape::Annule => {
            // l'utilisateur a quittet l'ecran, il n'y a plus rien a afficher
            siv.call_on_name("etape", |v: &mut TextView| v.set_content("Annule."));
        } // fin de l'annulation
    } // fin du traitement de l'evenement
} // fin maj_ecran

fn ecran_mesure(siv: &mut Cursive, demo: Demo, n_threads: usize, reps: usize, tx: Sender<Etape>) {
    // affiche le titre, la barre de progression, puis le rapport
    ANNULATION.store(false, Ordering::Relaxed); // nouvelle mesure, on repart de zero
    let titre = format!(" {} ", demo.nom()); // ex: " 4. Tri d'un tableau (1 M) "
    let detail = format!(" {} ", demo.detail()); // la phrase qui explique l'exemple
    let contexte = format!(" {n_threads} threads | {reps} repetitions par variante ",); // le contexte de mesure
    let vue = LinearLayout::vertical()
        .child(TextView::new(titre).style(Effect::Bold)) // titre de la demonstration
        .child(TextView::new(detail).style(Effect::Dim)) // rappel du principe utilise
        .child(TextView::new(contexte).style(Effect::Dim)) // threads et repetitions
        .child(TextView::new("Preparation...").with_name("etape")) // libelle de l'avancement
        .child(ProgressBar::new().with_name("barre")) // barre qui bouge toute seule
        .child(TextView::new("").with_name("resultats")) // le rapport, vide au depart
        .child(TextView::new(" Entree = retour au menu    q = quitter ").style(Effect::Dim)); // aide
    let couche = OnEventView::new(vue)
        .on_event(Key::Enter, |siv: &mut Cursive| {
            // Entree : on quitte l'ecran et on previent le worker
            ANNULATION.store(true, Ordering::Relaxed); // le worker s'arrete a la repetition suivante
            siv.pop_layer(); // retour au menu
        }) // fin du retour au menu
        .on_event('q', |siv: &mut Cursive| {
            // q : on quitte aussi le programme
            ANNULATION.store(true, Ordering::Relaxed); // le worker s'arrete
            siv.quit(); // fermeture complete
        }); // fin de la touche q
    siv.add_fullscreen_layer(couche.fixed_size((72, 16))); // l'ecran occupe tout le terminal
    // le worker part dans son propre thread, il ne touche jamais a l'interface
    std::thread::spawn(move || lancer(demo, n_threads, reps, tx)); // le canal tx lui sert a parler a l'ecran
} // fin ecran_mesure

fn lancer_depuis_le_menu(siv: &mut Cursive, tx: &Sender<Etape>) {
    // lit les 3 champs du menu puis ouvre l'ecran de mesure
    let detectes = nb_threads(); // coeurs detectes, sert de valeur par defaut
    // selection() renvoie un Option<Arc<Demo>> : le map() enleve le Arc, le flatten() retire l'absence
    let demo = siv
        .call_on_name("demos", |v: &mut SelectView<Demo>| v.selection().map(|d| *d))
        .flatten()
        .unwrap_or(Demo::Addition); // la demo choisie, addition par defaut
    let n_threads = siv
        .call_on_name("threads", |v: &mut SelectView<usize>| v.selection().map(|t| *t))
        .flatten()
        .unwrap_or(detectes); // le nombre de threads choisi
    // parse : texte -> nombre ; unwrap_or(5) si l'utilisateur a vide le champ
    let reps = siv
        .call_on_name("reps", |v: &mut EditView| v.get_content().trim().parse::<usize>().unwrap_or(5))
        .unwrap_or(5)
        .clamp(1, 50); // on borne entre 1 et 50 repetitions
    ecran_mesure(siv, demo, n_threads, reps, tx.clone()); // on part sur la mesure
} // fin lancer_depuis_le_menu

fn ecran_menu(siv: &mut Cursive, tx: &Sender<Etape>) {
    // ecran de depart : on choisit la demonstration, les threads et les repetitions
    let detectes = nb_threads(); // coeurs logiques de la machine, ex 8
    let max_threads = (detectes * 2).clamp(4, 32); // on propose jusqu'au double des coeurs
    let mut demos = SelectView::<Demo>::new(); // liste deroulante des demonstrations
    for d in Demo::tous() {
        demos.add_item(d.nom().to_string(), d); // un item par demonstration
    } // fin de la liste
    let mut threads = SelectView::<usize>::new(); // liste deroulante du nombre de threads
    for t in 1..=max_threads {
        threads.add_item(format!("{t} thread(s)"), t); // de 1 au double des coeurs
    } // fin de la liste
    // selected() est un constructeur : il renvoie la liste avec la ligne deja posee
    let threads = threads.selected((detectes - 1).min(max_threads - 1)); // defaut = nombre de coeurs
    let repetitions = EditView::new().content("5").with_name("reps").fixed_width(8); // 5 repetitions par defaut
    let tx_bouton = tx.clone(); // le bouton doit posseder son propre exemplaire du canal
    let bouton = Button::new(" Lancer la mesure ", move |siv: &mut Cursive| {
        // clic sur le bouton = lancement direct
        lancer_depuis_le_menu(siv, &tx_bouton);
    }); // fin du bouton
    let vue = LinearLayout::vertical()
        .child(TextView::new(" Mini-calculateur parallele : Rust + rayon ").style(Effect::Bold)) // titre
        .child(TextView::new(format!(" Coeurs detectes : {detectes} ")).style(Effect::Dim)) // rappel du materiel
        .child(TextView::new(" 1. Demonstration ")) // etape 1
        .child(Panel::new(ScrollView::new(demos.with_name("demos")).fixed_height(5))) // liste scrollable
        .child(TextView::new(" 2. Nombre de threads ")) // etape 2
        .child(Panel::new(ScrollView::new(threads.with_name("threads")).fixed_height(3))) // liste scrollable
        .child(TextView::new(" 3. Repetitions par variante ")) // etape 3
        .child(repetitions) // champ a remplir
        .child(bouton) // bouton de lancement
        .child(TextView::new(" l = lancer la mesure    fleches + Entree = choisir une valeur ").style(Effect::Dim)) // raccourcis
        .child(TextView::new(" Astuce : cargo run --release, sinon le tri est ~30x plus lent ").style(Effect::Dim)); // conseil
    siv.add_fullscreen_layer(vue.fixed_size((72, 21))); // le menu occupe tout le terminal
    siv.focus_name("demos").ok(); // le focus part de la liste des demonstrations
} // fin ecran_menu

fn main() {
    // point d'entree : on prepare l'interface puis on boucle sur les evenements
    let mut siv = cursive::default(); // cree l'application TUI (crossterm sur le terminal)
    let (tx, rx) = mpsc::channel::<Etape>(); // le worker envoie ses news par tx, l'interface les lit
    let reception = Mutex::new(rx); // Mutex : cursive exige un callback Send + Sync
    siv.set_fps(30); // 30 images par seconde : l'ecran se rafraichit tout seul
    // UNE SEULE callback globale, c'est elle qui fait vivre la barre de progression
    siv.add_global_callback(Event::Refresh, move |siv: &mut Cursive| {
        // appelee 30 fois par seconde : on vide le canal puis on met l'ecran a jour
        let mut news: Vec<Etape> = Vec::new(); // les evenements recus depuis le dernier rafraichissement
        {
            let garde = reception.lock().unwrap(); // on verrouille le canal le temps de le vider
            while let Ok(evenement) = garde.try_recv() {
                // try_recv = lit sans bloquer, vide quand il n'y a plus rien
                news.push(evenement); // on stocke l'evenement
            } // fin de la lecture du canal
        } // fin du verrou : on ne le garde pas pendant le dessin
        for evenement in news {
            maj_ecran(siv, evenement); // on applique chaque news a l'ecran
        } // fin de la mise a jour
    }); // fin de la callback globale
    siv.add_global_callback(Event::Char('q'), |siv: &mut Cursive| {
        // q ferme le programme depuis n'importe quel ecran
        ANNULATION.store(true, Ordering::Relaxed); // le worker s'arrete
        siv.quit(); // fermeture
    }); // fin de la touche q globale
    let tx_raccourci = tx.clone(); // la touche l a besoin du canal, comme le bouton
    siv.add_global_callback(Event::Char('l'), move |siv: &mut Cursive| {
        // l = lancer la mesure sans passer par le bouton (fonctionne meme au clavier seul)
        if siv.call_on_name("demos", |_v: &mut SelectView<Demo>| ()).is_some() {
            // on est bien sur le menu : les 3 champs existent
            lancer_depuis_le_menu(siv, &tx_raccourci);
        } // sinon on est sur l'ecran de mesure, la touche ne fait rien
    }); // fin de la touche l globale
    ecran_menu(&mut siv, &tx); // on demarre sur le menu
    siv.run(); // boucle d'evenements : le programme tourne ici tant qu'on ne quitte pas
} // fin main