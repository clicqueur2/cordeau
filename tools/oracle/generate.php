<?php
/**
 * generate.php — genere `crates/money/tests/fixtures/oracle.json`.
 *
 * Les 200 cas sont deterministes (aucun alea) et construits par balayage
 * arithmetique des pools ci-dessous, ce qui garantit que chaque quantite,
 * chaque prix, chaque remise, chaque taux et chaque combinaison de drapeaux
 * apparait au moins une fois.
 *
 * Usage :
 *   php tools/oracle/generate.php [options] [fichier_de_sortie]
 *
 *   --count=N        nombre de cas (defaut 200)
 *   --mode=balayage  balayage arithmetique deterministe (defaut)
 *   --mode=aleatoire tirage pseudo-aleatoire deterministe (--seed=N, defaut 1)
 *   --out=FICHIER    fichier de sortie
 *
 * Le fichier commite (200 cas, balayage) s'obtient sans option. Les modes
 * servent aux campagnes de stress ponctuelles, a jouer hors fixture :
 *   php tools/oracle/generate.php --mode=aleatoire --count=5000 --out=%TEMP%/stress.json
 *   $env:ORACLE_FIXTURE=".../stress.json"; cargo test -p money --test oracle
 */

declare(strict_types=1);

define('ORACLE_LIBRARY', true);
require __DIR__ . '/oracle.php';

$options = getopt('', array('count::', 'mode::', 'seed::', 'out::'));
$nombreDeCas = isset($options['count']) ? (int) $options['count'] : 200;
$mode = $options['mode'] ?? 'balayage';
$graine = isset($options['seed']) ? (int) $options['seed'] : 1;

// --- Pools -----------------------------------------------------------------
$QTY = array(
    '1', '2', '3', '5', '7', '10', '12', '25', '50', '100',
    '250', '1000', '0.5', '1.5', '2.25', '0.001', '0.01', '3.75', '12.5', '100000',
    '-2', '-0.5', // avoirs : quantites negatives
);
$PU = array(
    '1', '10', '100', '0.01', '0.005', '4.99', '19.99', '33.33', '1234.5678',
    '0.125', '2.675', '12.345', '99.995', '7.5', '0.1', '1000.001', '45678.91', '0.333',
);
$REMISE = array('0', '5', '10', '33.33', '100', '12.5');
$TVA = array('0', '5.5', '10', '20');

/**
 * Generateur congruentiel lineaire deterministe (pas de dependance, meme
 * suite a chaque execution pour une graine donnee).
 */
function oracle_lcg(int &$etat, int $max): int
{
    $etat = (int) (($etat * 1103515245 + 12345) % 2147483648);
    return intdiv($etat, 65536) % $max;
}

$cas = array();
if ($mode === 'aleatoire') {
    // Tirage large : quantites et prix a 0-4 decimales, remises a 2 decimales.
    $etat = $graine;
    for ($k = 0; $k < $nombreDeCas; $k++) {
        $qty = $QTY[oracle_lcg($etat, count($QTY))];
        $pu = $PU[oracle_lcg($etat, count($PU))];
        $remise = $REMISE[oracle_lcg($etat, count($REMISE))];
        $tva = $TVA[oracle_lcg($etat, count($TVA))];
        // Un tiers des cas : valeurs continues pour sortir des pools.
        if (oracle_lcg($etat, 3) === 0) {
            $qty = sprintf('%.3F', oracle_lcg($etat, 2000000) / 1000);
        }
        if (oracle_lcg($etat, 3) === 0) {
            $pu = sprintf('%.4F', oracle_lcg($etat, 5000000) / 1000);
        }
        $cas[] = array(
            'qty' => $qty,
            'pu' => $pu,
            'remise_percent' => $remise,
            'tva_tx' => $tva,
            'price_base_type' => oracle_lcg($etat, 2) === 0 ? 'HT' : 'TTC',
            'info_bits' => oracle_lcg($etat, 7) === 0 ? 1 : 0,
        );
    }
} else {
    // Balayage a pas premiers entre eux avec la taille des pools : toutes les
    // valeurs sortent, et aucune combinaison ne se repete sur 200 tirages.
    for ($k = 0; $k < $nombreDeCas; $k++) {
        $cas[] = array(
            'qty' => $QTY[$k % count($QTY)],
            'pu' => $PU[($k * 11) % count($PU)],
            'remise_percent' => $REMISE[($k * 5) % count($REMISE)],
            'tva_tx' => $TVA[($k * 3) % count($TVA)],
            'price_base_type' => ($k % 2 === 0) ? 'HT' : 'TTC',
            'info_bits' => ($k % 7 === 0) ? 1 : 0,
        );
    }
}

// --- Appel de l'oracle ------------------------------------------------------
$resultats = array();
foreach ($cas as $c) {
    $resultats[] = oracle_run($c);
}

$fixture = array(
    'oracle' => array(
        'source' => 'Dolibarr 24.0.1 (_vendor/dolibarr-src), core/lib/price.lib.php',
        'fonction' => 'calcul_price_total($qty, $pu, $remise, $txtva, 0, 0, 0, $base, $info_bits, 0)',
        'normalisation' => 'core/lib/functions.lib.php::price2num',
        'php' => PHP_VERSION,
        'MAIN_MAX_DECIMALS_UNIT' => (string) $GLOBALS['conf']->global->MAIN_MAX_DECIMALS_UNIT,
        'MAIN_MAX_DECIMALS_TOT' => (string) $GLOBALS['conf']->global->MAIN_MAX_DECIMALS_TOT,
        'MAIN_ROUNDING_RULE_TOT' => 'non defini',
        'localtax' => 'aucune (uselocaltax1_rate = uselocaltax2_rate = 0)',
        'nb_cas' => count($resultats),
        'genere_le' => gmdate('Y-m-d'),
        'regenerer' => 'php tools/oracle/generate.php',
    ),
    'cas' => $resultats,
);

$sortie = $options['out'] ?? ($argv[1] ?? (dirname(__DIR__, 2) . '/crates/money/tests/fixtures/oracle.json'));
$dossier = dirname($sortie);
if (!is_dir($dossier) && !mkdir($dossier, 0777, true) && !is_dir($dossier)) {
    fwrite(STDERR, "impossible de creer $dossier\n");
    exit(1);
}

$json = json_encode($fixture, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
if ($json === false) {
    fwrite(STDERR, 'json_encode: ' . json_last_error_msg() . "\n");
    exit(1);
}
file_put_contents($sortie, $json . "\n");

fwrite(STDERR, sprintf("%d cas ecrits dans %s\n", count($resultats), $sortie));
