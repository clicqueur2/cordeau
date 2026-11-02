<?php
/**
 * oracle.php — harnais de verite pour l'arithmetique monetaire de Dolibarr.
 *
 * Charge les sources OFFICIELLES de Dolibarr 24.0.1 (`price.lib.php` et
 * `functions.lib.php`) sans les modifier, bouchonne les globales dont elles ont
 * besoin (`$conf`, `$mysoc`, `$db`, `$hookmanager`) et expose la vraie fonction
 * `calcul_price_total()` en JSON.
 *
 * Utilisation CLI :
 *   echo '{"qty":"7","pu":"33.33",...}' | php tools/oracle/oracle.php
 *   echo '[{...},{...}]'               | php tools/oracle/oracle.php
 * Sortie : meme forme (objet ou tableau) avec `entree` + `attendu`.
 *
 * Mode bibliotheque : definir ORACLE_LIBRARY avant le require.
 *
 * Les fichiers amont sont des sources officielles : elles restent intactes.
 * Tout ce qui manque est bouchonne ici.
 */

declare(strict_types=1);

// ---------------------------------------------------------------------------
// Constantes que Dolibarr definit normalement dans main.inc.php / filefunc.inc.php
// ---------------------------------------------------------------------------
foreach (array('LOG_EMERG' => 0, 'LOG_ALERT' => 1, 'LOG_CRIT' => 2, 'LOG_ERR' => 3,
               'LOG_WARNING' => 4, 'LOG_NOTICE' => 5, 'LOG_INFO' => 6, 'LOG_DEBUG' => 7) as $k => $v) {
    if (!defined($k)) {
        define($k, $v);
    }
}

// Les sorties d'erreur vont sur stderr pour ne jamais polluer le JSON de stdout.
ini_set('display_errors', 'stderr');
error_reporting(E_ALL);

if (!defined('MAIN_DB_PREFIX')) {
    define('MAIN_DB_PREFIX', 'llx_');   // lu par la branche "deprecated" des localtaxes
}
if (!defined('DOL_DOCUMENT_ROOT')) {
    define('DOL_DOCUMENT_ROOT', dirname(__DIR__, 2) . '/_vendor/dolibarr-src/htdocs');
}

$ORACLE_ROOT = dirname(__DIR__, 2);
$ORACLE_LIBDIR = $ORACLE_ROOT . '/_vendor/dolibarr-src/htdocs/core/lib';

// ---------------------------------------------------------------------------
// Bouchons (aucune globale de Dolibarr n'est chargee : pas de base, pas de session)
// ---------------------------------------------------------------------------

/** Bouchon de `$hookmanager` : executeHooks() ne fait jamais rien. */
class OracleHookManagerStub
{
    public $resArray = array();
    public $resPrint = '';
    public $resErrors = array();

    public function executeHooks($method, $parameters = array(), $object = null, $action = '', $hookmanager = null)
    {
        return 0;
    }
}

/**
 * Bouchon de `$db`. `calcul_price_total()` n'appelle la base que dans sa branche
 * "deprecated" (recherche des localtaxes dans llx_c_tva quand `$localtaxes_array`
 * est vide), et seulement pour recuperer des localtaxes. On renvoie une requete
 * "valide" sans ligne : la fonction garde ses valeurs par defaut (type 0 = aucune
 * localtax appliquee), ce qui est exactement le perimetre testé (France, sans
 * localtax), et cela evite un `dol_print_error()` parasite.
 */
class OracleDbStub
{
    public function query($sql)
    {
        return new OracleResultSetStub();
    }

    public function fetch_object($resultset)
    {
        return null;
    }

    public function lasterror()
    {
        return '';
    }

    public function error()
    {
        return '';
    }
}

class OracleResultSetStub
{
}

/**
 * Prepare les globales attendues par les sources amont puis charge ces sources.
 * Idempotent : un seul chargement par processus.
 */
function oracle_bootstrap(): void
{
    global $ORACLE_LIBDIR, $conf, $mysoc, $db, $hookmanager, $langs, $user, $mc;

    if (defined('ORACLE_BOOTSTRAPPED')) {
        return;
    }

    // $langs reste NULL : price2num() bascule alors sur les separateurs anglais
    // ('.' decimal, ',' milliers), qui est exactement ce qu'on veut.
    $langs = null;

    // --- $conf -------------------------------------------------------------
    $conf = new stdClass();
    $conf->entity = 1;
    $conf->modules = array();          // aucun module : dol_syslog() sort immediatement
    $conf->logbuffer = array();
    $conf->loghandlers = array();
    $conf->global = new stdClass();

    // Precision de price2num(), identique au contrat du crate `money` :
    //   'MU' (prix unitaires) -> 6 decimales,
    //   'MT' (totaux)         -> 2 decimales.
    // Surchargeables par l'environnement pour mesurer l'impact d'un autre reglage.
    $conf->global->MAIN_MAX_DECIMALS_UNIT = getenv('ORACLE_MAX_DECIMALS_UNIT') !== false
        ? (string) getenv('ORACLE_MAX_DECIMALS_UNIT') : '6';
    $conf->global->MAIN_MAX_DECIMALS_TOT = getenv('ORACLE_MAX_DECIMALS_TOT') !== false
        ? (string) getenv('ORACLE_MAX_DECIMALS_TOT') : '2';
    // MAIN_ROUNDING_RULE_TOT volontairement absent : pas d'arrondi "base 10".
    // MAIN_APPLY_DISCOUNT_ON_UNIT_PRICE_THEN_ROUND_BEFORE_MULTIPLICATION_BY_QTY
    // volontairement absent : remise appliquee sur le total (regle par defaut).

    // --- $mysoc ------------------------------------------------------------
    // Seul `country_id` est lu par calcul_price_total() en dehors des localtaxes.
    $mysoc = new stdClass();
    $mysoc->id = 1;
    $mysoc->country_id = 1;
    $mysoc->country_code = 'FR';
    $mysoc->localtax1_assuj = 0;
    $mysoc->localtax2_assuj = 0;

    $db = new OracleDbStub();
    $hookmanager = new OracleHookManagerStub();
    $user = null;
    $mc = null;

    require_once $ORACLE_LIBDIR . '/functions.lib.php';
    require_once $ORACLE_LIBDIR . '/price.lib.php';

    define('ORACLE_BOOTSTRAPPED', true);
}

/**
 * Normalise un montant renvoye par calcul_price_total() en chaine decimale
 * canonique a 6 decimales (les sorties de price2num sont des chaines).
 */
function oracle_amount($v): string
{
    if (is_string($v)) {
        $v = trim($v);
        if ($v === '' || !is_numeric($v)) {
            throw new RuntimeException('sortie non numerique: ' . var_export($v, true));
        }
        $v = (float) $v;
    }
    // 6 decimales suffisent : price2num() arrondit au plus a 6 ('MU').
    $s = sprintf('%.6F', (float) $v);
    if ($s === '-0.000000') {
        $s = '0.000000';
    }
    return $s;
}

/**
 * Appelle la vraie fonction Dolibarr et renvoie les sorties utiles.
 *
 * @param array{qty:string,pu:string,remise_percent:string,tva_tx:string,price_base_type:string,info_bits:int} $case
 * @return array{total_ht:string,total_tva:string,total_ttc:string,pu_ht:string}
 */
function oracle_calc(array $case): array
{
    oracle_bootstrap();

    $qty = (string) $case['qty'];
    $pu = (string) $case['pu'];
    $remise = (string) ($case['remise_percent'] ?? '0');
    $tva = (string) ($case['tva_tx'] ?? '0');
    $base = (string) ($case['price_base_type'] ?? 'HT');
    $info_bits = (int) ($case['info_bits'] ?? 0);
    $type = (int) ($case['type'] ?? 0);

    // Signature exacte demandee : 10 arguments, seller/localtaxes_array par defaut.
    $r = calcul_price_total(
        (float) $qty,
        (float) $pu,
        (float) $remise,
        (float) $tva,
        0,      // uselocaltax1_rate : pas de localtax1
        0,      // uselocaltax2_rate : pas de localtax2
        0,
        $base,
        $info_bits,
        $type
    );

    if (!is_array($r) || !array_key_exists(0, $r)) {
        throw new RuntimeException('calcul_price_total() a renvoye un tableau vide');
    }

    return array(
        'total_ht' => oracle_amount($r[0]),
        'total_tva' => oracle_amount($r[1]),
        'total_ttc' => oracle_amount($r[2]),
        'pu_ht' => oracle_amount($r[3]),
    );
}

/** Traite un cas d'entree (objet JSON) et renvoie le bloc de sortie complet. */
function oracle_run(array $case): array
{
    $out = array(
        'entree' => array(
            'qty' => (string) $case['qty'],
            'pu' => (string) $case['pu'],
            'remise_percent' => (string) ($case['remise_percent'] ?? '0'),
            'tva_tx' => (string) ($case['tva_tx'] ?? '0'),
            'price_base_type' => (string) ($case['price_base_type'] ?? 'HT'),
            'info_bits' => (int) ($case['info_bits'] ?? 0),
        ),
        'attendu' => oracle_calc($case),
    );
    if (!empty($case['type'])) {
        $out['entree']['type'] = (int) $case['type'];
    }
    if (!empty($case['exclu'])) {
        $out['exclu'] = (string) $case['exclu'];
    }
    return $out;
}

// ---------------------------------------------------------------------------
// Mode CLI : lecture d'un JSON sur stdin (objet seul ou tableau)
// ---------------------------------------------------------------------------
if (!defined('ORACLE_LIBRARY')) {
    $raw = stream_get_contents(STDIN);
    if ($raw === false || trim($raw) === '') {
        fwrite(STDERR, "oracle.php: JSON attendu sur stdin\n");
        exit(2);
    }
    $data = json_decode($raw, true);
    if (!is_array($data)) {
        fwrite(STDERR, "oracle.php: JSON invalide (" . json_last_error_msg() . ")\n");
        exit(2);
    }

    $isList = array_keys($data) === range(0, count($data) - 1);

    try {
        if ($isList) {
            $out = array();
            foreach ($data as $case) {
                $out[] = oracle_run($case);
            }
        } else {
            $out = oracle_run($data);
        }
    } catch (Throwable $e) {
        fwrite(STDERR, 'oracle.php: ' . $e->getMessage() . "\n");
        exit(1);
    }

    echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE), "\n";
}
