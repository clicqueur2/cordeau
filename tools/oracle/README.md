# Oracle monetaire — `calcul_price_total()`

Verite de reference pour `crates/money` : les montants calcules en Rust sont
confrontes a la **vraie** fonction PHP de Dolibarr, sans reecriture ni
reimplementation de notre cote.

```
tools/oracle/
  oracle.php     harnais : bouchonne $conf/$mysoc/$db/$hookmanager, charge
                 _vendor/dolibarr-src/htdocs/core/lib/{functions,price}.lib.php
                 et expose calcul_price_total() en JSON
  generate.php   genere la fixture (200 cas deterministes par defaut)
crates/money/tests/
  oracle.rs              rejoue la fixture, tolerance zero
  fixtures/oracle.json   reference commitee (aucun besoin de PHP ensuite)
```

## Usage

```powershell
# un cas / un lot, sur stdin
'{"qty":"7","pu":"33.33","remise_percent":"5","tva_tx":"20","price_base_type":"HT","info_bits":0}' |
  php tools/oracle/oracle.php

# regenerer la fixture commitee
php tools/oracle/generate.php

# tester
$env:CARGO_TARGET_DIR="C:/Users/Maxime/.cargo-target/ag-oracle"
cargo test -p money
```

## Campagnes de stress (hors fixture)

```powershell
php tools/oracle/generate.php --mode=aleatoire --count=5000 --seed=7 --out=$env:TEMP\stress.json
$env:ORACLE_FIXTURE="$env:TEMP\stress.json"
cargo test -p money --test oracle -- --nocapture
```

Lancer ces campagnes quand on touche a `line_totals()` : la fixture commitee
(200 cas) protege de la regression, la campagne large sert a chercher les
frontieres de centime.

## Reglages de l'oracle

| Reglage | Valeur | Pourquoi |
|---|---|---|
| `MAIN_MAX_DECIMALS_TOT` | `2` | `price2num($x,'MT')` = totaux au centime, contrat de `round_money()` |
| `MAIN_MAX_DECIMALS_UNIT` | `6` | `price2num($x,'MU')` = prix unitaires a 6 dec., contrat de `round_unit_price()` |
| `MAIN_ROUNDING_RULE_TOT` | non defini | pas d'arrondi « base 10 » |
| `$langs` | `null` | `price2num()` bascule sur les separateurs anglais (`.` / `,`) |
| localtaxes | `0` / `0` | perimetre France : aucune localtax |
| `$progress`, `devise` | `100`, `1` | ligne simple, pas de multidevise |

Les deux constantes de precision sont surchargeables par l'environnement
(`ORACLE_MAX_DECIMALS_UNIT`, `ORACLE_MAX_DECIMALS_TOT`) pour mesurer l'impact
d'un autre reglage d'instance ; la fixture commitee documente celles utilisees.

## Pieges verifies

- `price2num()` renvoie une **chaine**, pas un flottant : le harnais la
  normalise en decimal a 6 chiffres (`.6F`) pour que le JSON soit lisible par
  `rust_decimal` sans perte.
- `calcul_price_total()` interroge `llx_c_tva` quand `$localtaxes_array` est
  vide (branche « deprecated ») : le bouchon `$db` renvoie une requete valide
  sans ligne, donc aucune localtax et aucun `dol_print_error()`.
- `dol_syslog()` sort immediatement car `$conf->modules` est vide ; `LOG_*`
  sont definis car ils apparaissent en argument.
- Les fichiers amont sont des **sources officielles** : elles ne sont jamais
  modifiees, tout ce qui manque est bouchonne dans `oracle.php`.
