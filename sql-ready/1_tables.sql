create table llx_accounting_account
(
  rowid                     bigint AUTO_INCREMENT PRIMARY KEY,
  entity                    integer DEFAULT 1 NOT NULL,
  datec                     datetime,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_pcg_version            varchar(32)  NOT NULL,
  pcg_type                  varchar(60)  NOT NULL,
  account_number            varchar(32)  NOT NULL,
  account_parent            integer DEFAULT 0,
  label                     varchar(255) NOT NULL,
  labelshort				varchar(255) DEFAULT NULL,
  fk_accounting_category    integer      DEFAULT 0,
  fk_user_author            integer      DEFAULT NULL,
  fk_user_modif             integer      DEFAULT NULL,
  active                    tinyint      DEFAULT 1  NOT NULL,
  centralized				tinyint      DEFAULT 0  NOT NULL,
  reconcilable				tinyint      DEFAULT 0  NOT NULL,
  import_key                varchar(14),
  extraparams               varchar(255)
)ENGINE=innodb;
CREATE TABLE llx_accounting_analytic_account (
    rowid               integer         AUTO_INCREMENT PRIMARY KEY,
    label               varchar(255)    NOT NULL,
    code                varchar(32)     NOT NULL,
    description			text,
    fk_axis				integer			NOT NULL,
    active              integer         DEFAULT 1,
    entity              integer         DEFAULT 1 NOT NULL,
    date_start			date,
    date_end			date,
    datec               datetime,
    tms                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_user_author      integer         NOT NULL,
    fk_user_modif       integer,
    import_key          varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_accounting_analytic_axis (
    rowid               integer         AUTO_INCREMENT PRIMARY KEY,
    label               varchar(255)    NOT NULL,
    code                varchar(32)     NOT NULL,
    active              integer         DEFAULT 1,
    entity              integer         DEFAULT 1 NOT NULL,
    datec               datetime,
    tms                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_user_author      integer         NOT NULL,
    fk_user_modif       integer,
    import_key          varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_accounting_analytic_distribution (
	rowid				integer			AUTO_INCREMENT PRIMARY KEY,
	fk_source_line		integer			NOT NULL,
	sourcetype			varchar(32)		NOT NULL,
	fk_analytic_account integer			NOT NULL,
	percentage			real			DEFAULT 100,
	amount				double(24,8),
	entity				integer			DEFAULT 1,
	datec               datetime,
	tms                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_author      integer         NOT NULL,
	fk_user_modif       integer,
	import_key          varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_accounting_balance_snapshot (
	rowid              	integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
	entity             	integer DEFAULT 1 NOT NULL,
	fk_fiscalyear      	integer NOT NULL,
	account_number     	varchar(32) NOT NULL,
	account_label		varchar(255) NOT NULL,
	subledger_account  	varchar(32),
	subledger_label    	varchar(255),
	debit              	double(24,8) NOT NULL default 0,
	credit             	double(24,8) NOT NULL default 0,
	date_snapshot      	datetime,
	tms                	timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=innodb;
CREATE TABLE llx_accounting_bookkeeping
(
  rowid                 integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  entity                integer DEFAULT 1 NOT NULL,
  ref             		VARCHAR(30),
  piece_num             integer NOT NULL,
  doc_date              date NOT NULL,
  doc_type              varchar(30) NOT NULL,
  doc_ref               varchar(300) NOT NULL,
  fk_doc                integer NOT NULL,
  fk_docdet             integer NOT NULL,
  thirdparty_code       varchar(32),
  subledger_account     varchar(32),
  subledger_label       varchar(255),
  numero_compte         varchar(32) NOT NULL,
  label_compte          varchar(255) NOT NULL,
  label_operation       varchar(255),
  debit                 double(24,8) NOT NULL,
  credit                double(24,8) NOT NULL,
  montant               double(24,8) NULL,
  sens                  varchar(1) DEFAULT NULL,
  multicurrency_amount  double(24,8),
  multicurrency_code    varchar(255),
  matching_general      tinyint DEFAULT 0 NOT NULL,
  lettering_code        varchar(255),
  date_lettering        datetime,
  date_lim_reglement    datetime DEFAULT NULL,
  fk_user_author        integer NOT NULL,
  fk_user_modif         integer,
  date_creation         datetime,
  tms                   timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user               integer NULL,
  code_journal          varchar(32) NOT NULL,
  journal_label         varchar(255),
  date_validated        datetime,
  date_export	      	datetime DEFAULT NULL,
  import_key            varchar(14),
  extraparams           varchar(255)
) ENGINE=innodb;
CREATE TABLE llx_accounting_bookkeeping_piece
(
  rowid                 integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  entity                integer DEFAULT 1 NOT NULL,
  ref             		varchar(128),
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec					datetime,
  datep					date NOT NULL,
  statut				smallint DEFAULT 0,
  note_private			text,
  note_public			text,
  fk_user_author		integer,
  fk_user_modif			integer,
  fk_user_valid			integer,
  fk_user_closing		integer,
  import_key            varchar(14),
  extraparams           varchar(255)
) ENGINE=innodb;
CREATE TABLE llx_accounting_bookkeeping_tmp
(
  rowid                 integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  ref             		VARCHAR(30),
  entity                integer DEFAULT 1 NOT NULL,
  doc_date              date NOT NULL,
  doc_type              varchar(30) NOT NULL,
  doc_ref               varchar(300) NOT NULL,
  fk_doc                integer NOT NULL,
  fk_docdet             integer NOT NULL,
  thirdparty_code       varchar(32),
  subledger_account     varchar(32),
  subledger_label       varchar(255),
  numero_compte         varchar(32),
  label_compte          varchar(255) NOT NULL,
  label_operation       varchar(255),
  debit                 double(24,8) NOT NULL,
  credit                double(24,8) NOT NULL,
  montant               double(24,8) NOT NULL,
  sens                  varchar(1) DEFAULT NULL,
  multicurrency_amount  double(24,8),
  multicurrency_code    varchar(255),
  matching_general      tinyint DEFAULT 0 NOT NULL,
  lettering_code        varchar(255),
  date_lettering        datetime,
  date_lim_reglement    datetime,
  fk_user_author        integer NOT NULL,
  fk_user_modif         integer,
  date_creation         datetime,
  tms                   timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user               integer NULL,
  code_journal          varchar(32) NOT NULL,
  journal_label         varchar(255),
  piece_num             integer NOT NULL,
  date_validated        datetime,
  import_key            varchar(14),
  extraparams	        varchar(255)
) ENGINE=innodb;
CREATE TABLE llx_accounting_category_account
(
  rowid           			integer AUTO_INCREMENT PRIMARY KEY,
  fk_accounting_category	integer,
  fk_accounting_account		bigint
) ENGINE=innodb;
create table llx_accounting_fiscalyear
(
	rowid			integer AUTO_INCREMENT PRIMARY KEY,
	label			varchar(128) NOT NULL,
	date_start		date,
	date_end		date,
	statut			tinyint DEFAULT 0 NOT NULL,
	entity			integer DEFAULT 1 NOT NULL,
	datec			datetime NOT NULL,
	tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_author	integer DEFAULT NULL,
	fk_user_modif	integer DEFAULT NULL
)ENGINE=innodb;
create table llx_accounting_groups_account
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  fk_accounting_account		INTEGER NOT NULL,
  fk_c_accounting_category	INTEGER NOT NULL
)ENGINE=innodb;
create table llx_accounting_journal
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  entity            integer DEFAULT 1 NOT NULL,
  code              varchar(32) NOT NULL,
  label             varchar(128) NOT NULL,
  nature            smallint DEFAULT 1 NOT NULL,
  active            smallint DEFAULT 0
)ENGINE=innodb;
create table llx_accounting_system
(
  rowid             integer         AUTO_INCREMENT PRIMARY KEY,
  fk_country		integer,
  pcg_version       varchar(32)     NOT NULL,
  label             varchar(128)    NOT NULL,
  active            smallint        DEFAULT 0,
  date_creation     datetime,
  fk_user_author    integer
)ENGINE=innodb;
CREATE TABLE llx_accounting_transaction_template (
	rowid			integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity          integer DEFAULT 1 NOT NULL,
	code			varchar(128) NOT NULL,
	label			varchar(255),
	date_creation	datetime NOT NULL,
	tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat	integer NOT NULL,
	fk_user_modif	integer,
	import_key		varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_accounting_transaction_template_det (
	rowid					integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	fk_transaction_template	integer NOT NULL,
	general_account			varchar(32) NOT NULL,
	general_label			varchar(255) NOT NULL,
	subledger_account		varchar(32),
	subledger_label			varchar(255),
	operation_label			varchar(255),
	debit					double(24,8),
	credit					double(24,8)
) ENGINE=innodb;
create table llx_actioncomm
(
  id				integer AUTO_INCREMENT PRIMARY KEY,
  ref               varchar(30) NOT NULL,
  ref_ext			varchar(255),
  entity			integer DEFAULT 1 NOT NULL,
  datep				datetime,
  datep2			datetime,
  fk_action			integer,
  code				varchar(50) NULL,
  datec				datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_author	integer,
  fk_user_mod		integer,
  fk_project		integer,
  fk_task			integer,
  fk_soc			integer,
  fk_contact		integer,
  fk_parent			integer NOT NULL default 0,
  fk_user_action	integer,
  transparency      integer,
  priority			smallint,
  visibility		varchar(12) DEFAULT 'default',
  fulldayevent		smallint NOT NULL default 0,
  percent			smallint NOT NULL default 0,
  location			varchar(128),
  durationp			real,
  label				varchar(255) NOT NULL,
  note				mediumtext,
  calling_duration  integer,
  email_subject		varchar(255),
  email_msgid		varchar(255),
  email_from		varchar(255),
  email_sender		varchar(255),
  email_to			varchar(255),
  email_tocc		varchar(255),
  email_tobcc		varchar(255),
  errors_to			varchar(255),
  reply_to			varchar(255),
  recurid           varchar(128),
  recurrule         varchar(128),
  recurdateend      datetime,
  num_vote          integer DEFAULT NULL,
  event_paid        smallint NOT NULL DEFAULT 0,
  status            smallint NOT NULL DEFAULT 0,
  max_participants  integer DEFAULT NULL,
  fk_element		integer DEFAULT NULL,
  elementtype		varchar(255) DEFAULT NULL,
  ip            varchar(250),
  fk_bookcal_calendar integer DEFAULT NULL,
  import_key		varchar(14),
  extraparams		varchar(255)
)ENGINE=innodb;
create table llx_actioncomm_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_actioncomm_reminder(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	dateremind datetime NOT NULL,
	typeremind varchar(32) NOT NULL,
	fk_user integer NULL,
	fk_soc integer NULL,
	fk_contact integer NULL,
	offsetvalue integer NOT NULL,
	offsetunit varchar(1) NOT NULL,
	status integer NOT NULL DEFAULT 0,
	datedone datetime NULL,
	lasterror varchar(128) NULL,
	entity integer NOT NULL DEFAULT 1,
	fk_actioncomm integer NOT NULL,
	fk_email_template integer
) ENGINE=innodb;
create table llx_actioncomm_resources
(
  rowid           	integer AUTO_INCREMENT PRIMARY KEY,
  fk_actioncomm		integer NOT NULL,
  element_type		varchar(50) NOT NULL,
  fk_element		integer NOT NULL,
  answer_status		varchar(50) NULL,
  mandatory			smallint,
  transparency		smallint default 1
) ENGINE=innodb;
create table llx_adherent
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  ref              varchar(30) NOT NULL,
  entity           integer DEFAULT 1 NOT NULL,
  ref_ext          varchar(128),
  gender           varchar(10),
  civility         varchar(6),
  lastname         varchar(50),
  firstname        varchar(50),
  login            varchar(50),
  pass             varchar(50),
  pass_crypted     varchar(128),
  fk_adherent_type integer NOT NULL,
  morphy           varchar(3) NOT NULL,
  societe          varchar(128),
  fk_soc           integer NULL,
  address          text,
  zip              varchar(30),
  town             varchar(50),
  state_id         integer,
  country          integer,
  email            varchar(255),
  url              varchar(255) NULL,
  socialnetworks   text DEFAULT NULL,
  phone            varchar(30),
  phone_perso      varchar(30),
  phone_mobile     varchar(30),
  birth            date,
  birth_place      varchar(64),
  photo            varchar(255),
  statut           smallint NOT NULL DEFAULT 0,
  public           smallint NOT NULL DEFAULT 0,
  datefin          datetime,
  default_lang     varchar(6) DEFAULT NULL,
  note_private     text DEFAULT NULL,
  note_public      text DEFAULT NULL,
  model_pdf		     varchar(255),
  datevalid        datetime,
  datec            datetime,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_author   integer,
  fk_user_mod      integer,
  fk_user_valid    integer,
  canvas           varchar(32),
  ip               varchar(250),
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_adherent_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_adherent_type
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  entity           integer DEFAULT 1 NOT NULL,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  statut           smallint NOT NULL DEFAULT 0,
  libelle          varchar(50) NOT NULL,
  morphy           varchar(3) NOT NULL,
  duration         varchar(6) DEFAULT NULL,
  subscription     varchar(3) NOT NULL DEFAULT '1',
  amount           double(24,8) DEFAULT NULL,
  caneditamount    integer DEFAULT 0,
  minimumamount    double(24,8) DEFAULT NULL,
  amountformuladescription text,
  vote             varchar(3) NOT NULL DEFAULT '1',
  note             text,
  mail_valid       text
)ENGINE=innodb;
create table llx_adherent_type_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_adherent_type_lang
(
  rowid          integer AUTO_INCREMENT PRIMARY KEY,
  fk_type        integer      DEFAULT 0 NOT NULL,
  lang           varchar(5)   DEFAULT 0 NOT NULL,
  label          varchar(255) NOT NULL,
  description    text,
  email          text,
  import_key varchar(14) DEFAULT NULL
)ENGINE=innodb;
create table llx_ai_request_log
(
  rowid						integer AUTO_INCREMENT PRIMARY KEY,
  entity					integer DEFAULT 1 NOT NULL,
  date_request				datetime,
  fk_user					integer NOT NULL,
  fk_actioncomm				integer,
  query_text				text,
  tool_name					varchar(255),
  provider					varchar(50),
  execution_time			float,
  confidence				float,
  status					varchar(50),
  error_msg					text,
  input_hash				varchar(80),
  output_hash				varchar(80),
  security_hash				varchar(80),
  raw_request_payload		MEDIUMTEXT,
  raw_response_payload		MEDIUMTEXT
)ENGINE=innodb;
CREATE TABLE llx_asset(
    rowid                   integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    ref                     varchar(128)    NOT NULL,
    entity                  integer         DEFAULT 1 NOT NULL,
    label                   varchar(255),
    fk_asset_model          integer,
    reversal_amount_ht      double(24,8),
    acquisition_value_ht    double(24,8)    DEFAULT NULL,
    recovered_vat           double(24,8),
    reversal_date           date,
    date_acquisition        date            NOT NULL,
    date_start              date            NOT NULL,
    qty                     real            DEFAULT 1 NOT NULL,
    acquisition_type        smallint        DEFAULT 0 NOT NULL,
    asset_type              smallint        DEFAULT 0 NOT NULL,
    not_depreciated         boolean         DEFAULT false,
    disposal_date           date,
    disposal_amount_ht      double(24,8),
    fk_disposal_type        integer,
    disposal_depreciated    boolean         DEFAULT false,
    disposal_subject_to_vat boolean         DEFAULT false,
    note_public             text,
    note_private            text,
    date_creation           datetime        NOT NULL,
    tms                     timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    date_valid              datetime,
    fk_user_creat           integer         NOT NULL,
    fk_user_modif           integer,
    fk_user_valid           integer,
    last_main_doc           varchar(255),
    import_key              varchar(14),
    model_pdf               varchar(255),
    status                  integer         NOT NULL
) ENGINE=innodb;
CREATE TABLE llx_asset_accountancy_codes_economic(
    rowid						integer			AUTO_INCREMENT PRIMARY KEY NOT NULL,
    fk_asset					integer,
    fk_asset_model				integer,
    asset						varchar(32),
    depreciation_asset			varchar(32),
    depreciation_expense		varchar(32),
    value_asset_sold			varchar(32),
    receivable_on_assignment	varchar(32),
    proceeds_from_sales			varchar(32),
    vat_collected				varchar(32),
    vat_deductible				varchar(32),
    tms                         timestamp       DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_user_modif				integer
) ENGINE=innodb;
CREATE TABLE llx_asset_accountancy_codes_fiscal(
    rowid									integer			AUTO_INCREMENT PRIMARY KEY NOT NULL,
    fk_asset								integer,
    fk_asset_model							integer,
    accelerated_depreciation				varchar(32),
    endowment_accelerated_depreciation		varchar(32),
    provision_accelerated_depreciation		varchar(32),
    tms                                     timestamp       DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_user_modif							integer
) ENGINE=innodb;
CREATE TABLE llx_asset_depreciation(
    rowid                           integer         AUTO_INCREMENT PRIMARY KEY NOT NULL,
    fk_asset                        integer         NOT NULL,
    depreciation_mode               varchar(255)	NOT NULL,
    ref                             varchar(255)	NOT NULL,
    depreciation_date               datetime		NOT NULL,
    depreciation_ht                 double(24,8)	NOT NULL,
    cumulative_depreciation_ht      double(24,8)	NOT NULL,
    accountancy_code_debit          varchar(32),
    accountancy_code_credit         varchar(32),
    tms                             timestamp       DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_user_modif                   integer
) ENGINE=innodb;
CREATE TABLE llx_asset_depreciation_options_economic(
    rowid                               integer         AUTO_INCREMENT PRIMARY KEY NOT NULL,
    fk_asset							integer,
    fk_asset_model						integer,
    depreciation_type					smallint		DEFAULT 0 NOT NULL,
    accelerated_depreciation_option		boolean DEFAULT false,
    degressive_coefficient				double(24,8),
    duration							smallint		NOT NULL,
    duration_type						smallint		DEFAULT 0  NOT NULL,
	amount_base_depreciation_ht			double(24,8),
	amount_base_deductible_ht			double(24,8),
	total_amount_last_depreciation_ht	double(24,8),
	tms                                 timestamp       DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_modif						integer
) ENGINE=innodb;
CREATE TABLE llx_asset_depreciation_options_fiscal(
    rowid								integer			AUTO_INCREMENT PRIMARY KEY NOT NULL,
    fk_asset							integer,
    fk_asset_model						integer,
    depreciation_type					smallint		DEFAULT 0 NOT NULL,
    degressive_coefficient				double(24,8),
    duration							smallint		NOT NULL,
    duration_type						smallint		DEFAULT 0  NOT NULL,
	amount_base_depreciation_ht			double(24,8),
	amount_base_deductible_ht			double(24,8),
	total_amount_last_depreciation_ht	double(24,8),
	tms                                 timestamp       DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_modif						integer
) ENGINE=innodb;
create table llx_asset_extrafields
(
    rowid           integer     AUTO_INCREMENT PRIMARY KEY,
    tms             timestamp   DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object       integer     NOT NULL,
    import_key      varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_asset_model(
    rowid					integer			AUTO_INCREMENT PRIMARY KEY NOT NULL,
    entity					integer			DEFAULT 1 NOT NULL,
    ref						varchar(128)	NOT NULL,
    ref_ext                 varchar(255),
    label					varchar(255)	NOT NULL,
    asset_type				smallint		NOT NULL,
    fk_pays                 integer         DEFAULT 0,
    note_public				text,
    note_private			text,
    date_creation			datetime		NOT NULL,
    tms                     timestamp       DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_user_creat			integer			NOT NULL,
    fk_user_modif			integer,
    import_key				varchar(14),
    status					smallint		NOT NULL
) ENGINE=innodb;
CREATE TABLE llx_asset_model_extrafields
(
    rowid           integer     AUTO_INCREMENT PRIMARY KEY,
    tms             timestamp   DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object       integer     NOT NULL,
    import_key      varchar(14)
) ENGINE=innodb;
create table llx_bank
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  datec           datetime,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datev           date,
  dateo           date,
  amount          double(24,8) NOT NULL default 0,
  amount_main_currency double(24,8) NULL,
  label           varchar(255),
  fk_account      integer,
  fk_user_author  integer,
  fk_user_rappro  integer,
  fk_type         varchar(6),
  num_releve      varchar(50),
  num_chq         varchar(50),
  numero_compte   varchar(32) NULL,
  rappro          tinyint default 0,
  note            text,
  fk_bordereau    integer DEFAULT 0,
  position        integer DEFAULT 0,
  banque          varchar(255),
  emetteur        varchar(255),
  author          varchar(40),
  origin_id       integer,
  origin_type     varchar(64) NULL,
  import_key	  varchar(14)
)ENGINE=innodb;
create table llx_bank_account
(
  rowid						integer AUTO_INCREMENT PRIMARY KEY,
  datec						datetime,
  tms						timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  ref						varchar(12) NOT NULL,
  label						varchar(50) NOT NULL,
  entity					integer DEFAULT 1 NOT NULL,
  fk_user_author			integer,
  fk_user_modif				integer,
  bank						varchar(60),
  code_banque				varchar(128),
  code_guichet				varchar(6),
  number					varchar(255),
  cle_rib					varchar(5),
  bic						varchar(11),
  bic_intermediate          varchar(11),
  iban_prefix				varchar(100),
  country_iban				varchar(2),
  cle_iban					varchar(2),
  domiciliation				varchar(255),
  pti_in_ctti               smallint DEFAULT 0,
  state_id					integer        DEFAULT NULL,
  fk_pays					integer        NOT NULL,
  proprio					varchar(60),
  owner_address				varchar(255),
  owner_zip					varchar(25),
  owner_town				varchar(50),
  owner_country_id			integer DEFAULT NULL,
  courant					smallint DEFAULT 0 NOT NULL,
  clos						smallint DEFAULT 0 NOT NULL,
  rappro					smallint DEFAULT 1,
  url						varchar(128),
  account_number			varchar(32),
  fk_accountancy_journal	integer,
  currency_code				varchar(3) NOT NULL,
  min_allowed				integer DEFAULT 0,
  min_desired				integer DEFAULT 0,
  comment					text,
  note_public				text,
  model_pdf					varchar(255),
  import_key				varchar(14),
  extraparams			    varchar(255),
  ics						varchar(32),
  ics_transfer              varchar(32)
)ENGINE=innodb;
create table llx_bank_account_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_bank_extrafields
(
  rowid      integer AUTO_INCREMENT PRIMARY KEY,
  tms        timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object  integer NOT NULL,
  import_key varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_bank_import
(
  rowid                 integer         AUTO_INCREMENT PRIMARY KEY,
  id_account			integer			NOT NULL,
  fitid					varchar(255)	NULL,
  record_type 			varchar(64)   	NULL,
  label         		varchar(255)  	NOT NULL,
  record_type_origin  	varchar(255)  	NOT NULL,
  label_origin  		varchar(255)  	NOT NULL,
  comment				text			NULL,
  note				    text			NULL,
  bdate					date			NULL,
  vdate					date			NULL,
  date_scraped			datetime		NULL,
  original_amount		double(24,8)	NULL,
  original_currency		varchar(255)	NULL,
  amount_debit			double(24,8)	NOT NULL,
  amount_credit       	double(24,8)  NOT NULL,
  deleted_date			datetime		NULL,
  fk_duplicate_of		integer			NULL,
  status				smallint		NOT NULL,
  datec					datetime		NOT NULL,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_author	    integer         NOT NULL,
  fk_user_modif		    integer,
  import_key			varchar(14),
  datas					text			NOT NULL
)ENGINE=innodb;
CREATE TABLE llx_bank_record
(
  rowid             integer     AUTO_INCREMENT PRIMARY KEY,
  ref 				varchar(50) NOT NULL,
  fk_bank			integer		NOT NULL,
  dt_from			date		NOT NULL,
  dt_to				date		NOT NULL,
  date_creation datetime NOT NULL,
  date_valid datetime NULL,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
CREATE TABLE llx_bank_record_link
(
  rowid             integer     AUTO_INCREMENT PRIMARY KEY,
  fk_bank_record	integer		NOT NULL,
  fk_bank_import	integer		NOT NULL
)ENGINE=innodb;
create table llx_bank_url
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  fk_bank         integer,
  url_id          integer,
  url             varchar(255),
  label           varchar(255),
  type            varchar(24) NOT NULL
)ENGINE=innodb;
CREATE TABLE llx_blockedlog
(
	rowid integer AUTO_INCREMENT PRIMARY KEY,
	module_source varchar(32) DEFAULT '',
	pos_source varchar(32) DEFAULT '',
	action varchar(50),
	entity integer DEFAULT 1 NOT NULL,
	date_creation	datetime,
	amounts double(24,8) NOT NULL,
	amounts_taxexcl double(24,8) NULL,
	ref_object varchar(255),
	date_object	datetime,
	type_code varchar(8) DEFAULT '',
	user_fullname varchar(255),
	linktoref text,
	linktype varchar(16),
	object_data	mediumtext,
	signature varchar(100) NOT NULL,
	signature_backward varchar(100) DEFAULT '',
	element varchar(50),
	fk_user	integer,
	fk_object integer,
	object_version varchar(32) DEFAULT '',
	object_format varchar(16) DEFAULT 'V1',
	certified integer,
	actionrefisunique varchar(16) DEFAULT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	debuginfo mediumtext,
	note varchar(128) NULL
) ENGINE=innodb;
CREATE TABLE llx_bom_bom(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity integer DEFAULT 1 NOT NULL,
	ref varchar(128) NOT NULL,
	bomtype integer DEFAULT 0,
	label varchar(255),
	fk_product integer,
	description text,
	note_public text,
	note_private text,
	fk_warehouse integer,
	qty double(24,8),
	efficiency double(24,8) DEFAULT 1,
	duration double(24,8) DEFAULT NULL,
	date_creation datetime NOT NULL,
	date_valid datetime,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	fk_user_valid integer,
	import_key varchar(14),
	model_pdf varchar(255),
	last_main_doc varchar(255),
	status integer NOT NULL
) ENGINE=innodb;
create table llx_bom_bom_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_bom_bomline(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	fk_bom integer NOT NULL,
	fk_product integer NOT NULL,
	fk_bom_child integer NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	description text,
	import_key varchar(14),
	qty double(24,8) NOT NULL,
    qty_frozen smallint DEFAULT 0,
    disable_stock_change smallint DEFAULT 0,
	efficiency double(24,8) NOT NULL DEFAULT 1,
	fk_unit integer NULL,
	position integer NOT NULL DEFAULT 0,
	fk_default_workstation integer DEFAULT NULL
) ENGINE=innodb;
create table llx_bom_bomline_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_bookcal_availabilities(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	label varchar(255),
	description text,
	note_public text,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	last_main_doc varchar(255),
	import_key varchar(14),
	model_pdf varchar(255),
	status integer NOT NULL,
	start date NOT NULL,
	end date NOT NULL,
	duration integer DEFAULT 30 NOT NULL,
	startHour integer NOT NULL,
	endHour integer NOT NULL,
	fk_bookcal_calendar integer NOT NULL
) ENGINE=innodb;
create table llx_bookcal_availabilities_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_bookcal_calendar(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity integer DEFAULT 1 NOT NULL,
	ref varchar(128) NOT NULL,
	label varchar(255),
	fk_soc integer,
	fk_project integer,
	description text,
	note_public text,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	import_key varchar(14),
	status integer NOT NULL DEFAULT 0,
	type integer NOT NULL,
	visibility integer NOT NULL DEFAULT 1
) ENGINE=innodb;
create table llx_bookcal_calendar_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_bookmark
(
  rowid       integer AUTO_INCREMENT PRIMARY KEY,
  fk_user     integer NOT NULL,
  dateb       datetime,
  url         TEXT,
  target      varchar(16),
  title       varchar(64),
  favicon     varchar(24),
  position    integer DEFAULT 0,
  entity      integer DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_bordereau_cheque
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  ref               varchar(30) NOT NULL,
  label             varchar(255),
  ref_ext           varchar(255),
  type              varchar(6) DEFAULT 'CHQ',
  datec             datetime NOT NULL,
  date_bordereau    date,
  amount            double(24,8) NOT NULL,
  nbcheque          smallint NOT NULL,
  fk_bank_account   integer,
  fk_user_author    integer,
  statut            smallint NOT NULL DEFAULT 0,
  tms               timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  note              text,
  entity            integer DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_boxes
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  entity		integer NOT NULL DEFAULT 1,
  box_id		integer NOT NULL,
  position		smallint NOT NULL,
  box_order		varchar(3) NOT NULL,
  fk_user		integer default 0 NOT NULL,
  maxline		integer NULL,
  params		varchar(255)
)ENGINE=innodb;
create table llx_boxes_def
(
  rowid       integer AUTO_INCREMENT PRIMARY KEY,
  file        varchar(200) NOT NULL,
  entity      integer DEFAULT 1 NOT NULL,
  fk_user     integer DEFAULT 0 NOT NULL,
  tms         timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  note        varchar(130)
)ENGINE=innodb;
create table llx_budget
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  entity		integer NOT NULL DEFAULT 1,
  label         varchar(255) NOT NULL,
  status        integer,
  note			text,
  date_start	date,
  date_end		date,
  datec         datetime,
  tms           timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat integer,
  fk_user_modif integer,
  import_key    integer
)ENGINE=innodb;
create table llx_budget_lines
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  fk_budget     	integer NOT NULL,
  fk_project_ids	varchar(180) NOT NULL,
  amount			double(24,8) NOT NULL,
  datec        		datetime,
  tms           	timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat 	integer,
  fk_user_modif 	integer,
  import_key    	integer
)ENGINE=innodb;
CREATE TABLE llx_c_accounting_category (
  rowid 				integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  entity 				integer NOT NULL DEFAULT 1,
  fk_report				integer NOT NULL DEFAULT 1,
  code 					varchar(16) NOT NULL,
  label 				varchar(255) NOT NULL,
  range_account			varchar(255) NOT NULL,
  sens 					tinyint NOT NULL DEFAULT '0',
  category_type			tinyint NOT NULL DEFAULT '0',
  formula				varchar(255) NOT NULL,
  position    			integer DEFAULT 0,
  fk_country 			integer DEFAULT NULL,
  active 				integer DEFAULT 1
) ENGINE=innodb;
CREATE TABLE llx_c_accounting_report (
  rowid 				integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  entity 				integer NOT NULL DEFAULT 1,
  code 					varchar(16) NOT NULL,
  label 				varchar(255) NOT NULL,
  fk_country			integer DEFAULT NULL,
  active 				integer DEFAULT 1
) ENGINE=innodb;
create table llx_c_action_trigger
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  elementtype	varchar(64) 			NOT NULL,
  code			varchar(128)			NOT NULL,
  contexts		varchar(255)			NULL,
  label			varchar(128)			NOT NULL,
  description	varchar(255),
  rang			integer		DEFAULT 0
)ENGINE=innodb;
create table llx_c_actioncomm
(
  id         integer     PRIMARY KEY,
  code       varchar(50) NOT NULL,
  type       varchar(50) DEFAULT 'system' NOT NULL,
  libelle    varchar(128) NOT NULL,
  module	 varchar(50) DEFAULT NULL,
  active     tinyint DEFAULT 1 NOT NULL,
  todo       tinyint,
  color      varchar(9),
  picto      varchar(48),
  position   integer NOT NULL DEFAULT 0
)ENGINE=innodb;
CREATE TABLE llx_c_asset_disposal_type
(
    rowid    integer            AUTO_INCREMENT PRIMARY KEY,
    entity 	 integer            NOT NULL DEFAULT 1,
    code     varchar(16)        NOT NULL,
    label    varchar(50)        NOT NULL,
    active   integer            DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_availability
(
	rowid		integer	 	AUTO_INCREMENT PRIMARY KEY,
	code		varchar(30) NOT NULL,
	label		varchar(128) NOT NULL,
	type_duration	varchar(1),
	qty	    real,
	active		tinyint 	DEFAULT 1  NOT NULL,
	position    integer NOT NULL DEFAULT 0
)ENGINE=innodb;
create table llx_c_barcode_type
(
  rowid    integer            AUTO_INCREMENT PRIMARY KEY,
  code     varchar(16)        NOT NULL,
  entity   integer  DEFAULT 1 NOT NULL,
  libelle  varchar(128)        NOT NULL,
  coder    varchar(16)        NOT NULL,
  example  varchar(16)        NOT NULL
)ENGINE=innodb;
create table llx_c_chargesociales
(
  id           integer AUTO_INCREMENT PRIMARY KEY,
  libelle      varchar(128),
  deductible   smallint DEFAULT 0 NOT NULL,
  active       tinyint DEFAULT 1  NOT NULL,
  code         varchar(24) NOT NULL,
  accountancy_code varchar(32) DEFAULT NULL,
  fk_pays      integer DEFAULT 1 NOT NULL,
  module       varchar(32) NULL
)ENGINE=innodb;
create table llx_c_civility
(
  rowid       integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
  code        varchar(6) NOT NULL,
  label		  varchar(128),
  active      tinyint DEFAULT 1  NOT NULL,
  module      varchar(32) NULL
)ENGINE=innodb;
create table llx_c_country
(
  rowid    		integer            PRIMARY KEY,
  code     		varchar(2)         NOT NULL,
  code_iso 		varchar(3),
  numeric_code 	varchar(3),
  label    		varchar(128)       NOT NULL,
  eec      		tinyint DEFAULT 0  NOT NULL,
  sepa     		tinyint DEFAULT 0  NOT NULL,
  active   		tinyint DEFAULT 1  NOT NULL,
  favorite 		tinyint DEFAULT 0  NOT NULL,
  phone_code	integer,
  trunk_prefix	varchar(5)
)ENGINE=innodb;
create table llx_c_currencies
(
  code_iso			varchar(3)  PRIMARY KEY,
  label				varchar(128) NOT NULL,
  unicode			varchar(32) DEFAULT NULL,
  active			tinyint		DEFAULT 1  NOT NULL,
  max_decimal_unit 	tinyint 	NULL,
  max_decimal_tot 	tinyint 	NULL
)ENGINE=innodb;
create table llx_c_departements
(
  rowid            integer    AUTO_INCREMENT PRIMARY KEY,
  code_departement varchar(6) NOT NULL,
  fk_region        integer,
  cheflieu         varchar(50),
  tncc             integer,
  ncc              varchar(50),
  nom              varchar(50),
  active           tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_ecotaxe
(
  rowid			integer      AUTO_INCREMENT PRIMARY KEY,
  code			varchar(64)  NOT NULL,
  label			varchar(255),
  price			double(24,8),
  organization	varchar(255),
  fk_pays		integer NOT NULL,
  active		tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_effectif
(
  id      integer     PRIMARY KEY,
  code    varchar(12) NOT NULL,
  libelle varchar(128),
  active  tinyint DEFAULT 1  NOT NULL,
  module  varchar(32) NULL
)ENGINE=innodb;
create table llx_c_email_senderprofile
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  entity		  integer DEFAULT 1 NOT NULL,
  private         smallint DEFAULT 0 NOT NULL,
  date_creation   datetime,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  label           varchar(255),
  email           varchar(255) NOT NULL,
  signature		  text,
  position        smallint DEFAULT 0,
  active          tinyint DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_email_templates
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  entity		  integer DEFAULT 1 NOT NULL,
  module          varchar(32),
  type_template   varchar(32),
  lang            varchar(6) DEFAULT '',
  private         smallint DEFAULT 0 NOT NULL,
  fk_user         integer,
  datec           datetime,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  label           varchar(180),
  position        smallint,
  defaultfortype  smallint DEFAULT 0,
  enabled         varchar(255) DEFAULT '1',
  active          tinyint DEFAULT 1  NOT NULL,
  email_from	  varchar(255),
  email_to		  varchar(255),
  email_tocc	  varchar(255),
  email_tobcc	  varchar(255),
  topic			  text,
  joinfiles		  text,
  content         mediumtext,
  content_lines   text
)ENGINE=innodb;
CREATE TABLE IF NOT EXISTS llx_c_exp_tax_cat (
    rowid       integer  AUTO_INCREMENT PRIMARY KEY,
    label       varchar(128) NOT NULL,
    entity      integer DEFAULT 1 NOT NULL,
    active      integer DEFAULT 1 NOT NULL
)ENGINE=innodb;
CREATE TABLE IF NOT EXISTS llx_c_exp_tax_range (
    rowid       integer  AUTO_INCREMENT PRIMARY KEY,
    fk_c_exp_tax_cat integer DEFAULT 1 NOT NULL,
    range_ik    double DEFAULT 0 NOT NULL,
    entity      integer DEFAULT 1 NOT NULL,
    active      integer DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_field_list
(
  rowid			integer  AUTO_INCREMENT PRIMARY KEY,
  tms			timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  element		varchar(64)        			NOT NULL,
  entity		integer			DEFAULT 1 	NOT NULL,
  name			varchar(32)        			NOT NULL,
  alias			varchar(32)					NOT NULL,
  title			varchar(32)        			NOT NULL,
  align			varchar(6)		DEFAULT 'left',
  sort			tinyint 		DEFAULT 1  	NOT NULL,
  search		tinyint 		DEFAULT 0  	NOT NULL,
  visible		tinyint			DEFAULT 1	NOT NULL,
  enabled       varchar(255)	DEFAULT 1,
  rang      	integer 		DEFAULT 0
)ENGINE=innodb;
CREATE TABLE llx_c_format_cards
(
  rowid integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  code varchar(50) NOT NULL,
  name varchar(50) NOT NULL,
  paper_size varchar(20) NOT NULL,
  orientation varchar(1) NOT NULL,
  metric varchar(5) NOT NULL,
  leftmargin double(24,8) NOT NULL,
  topmargin double(24,8) NOT NULL,
  nx integer NOT NULL,
  ny integer NOT NULL,
  spacex double(24,8) NOT NULL,
  spacey double(24,8) NOT NULL,
  width double(24,8) NOT NULL,
  height double(24,8) NOT NULL,
  font_size integer NOT NULL,
  custom_x double(24,8) NOT NULL,
  custom_y double(24,8) NOT NULL,
  active integer NOT NULL
) ENGINE=innodb;
create table llx_c_forme_juridique
(
  rowid      	integer AUTO_INCREMENT PRIMARY KEY,
  code       	integer NOT NULL,
  fk_pays    	integer NOT NULL,
  libelle    	varchar(255),
  isvatexempted	tinyint DEFAULT 0  NOT NULL,
  active     	tinyint DEFAULT 1  NOT NULL,
  module        varchar(32) NULL,
  position      integer NOT NULL DEFAULT 0
)ENGINE=innodb;
CREATE TABLE llx_c_holiday_types
(
  rowid				integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  entity			integer DEFAULT 1 NOT NULL,
  code				varchar(16) NOT NULL,
  label				varchar(255) NOT NULL,
  affect			integer NOT NULL,
  delay				integer NOT NULL,
  newbymonth		double(8,5) DEFAULT 0 NOT NULL,
  fk_country		integer DEFAULT NULL,
  block_if_negative	integer NOT NULL DEFAULT 0,
  sortorder			smallint,
  active			integer DEFAULT 1
) ENGINE=innodb;
create table llx_c_hrm_department
(
  rowid      	integer     PRIMARY KEY,
  pos   		tinyint DEFAULT 0 NOT NULL,
  code    		varchar(16) NOT NULL,
  label 		varchar(128),
  active  		tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_hrm_function
(
  rowid     integer     PRIMARY KEY,
  pos   	tinyint DEFAULT 0 NOT NULL,
  code    	varchar(16) NOT NULL,
  label 	varchar(128),
  c_level   tinyint DEFAULT 0 NOT NULL,
  active  	tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_hrm_public_holiday
(
  id               integer AUTO_INCREMENT PRIMARY KEY,
  entity           integer	DEFAULT 1 NOT NULL,
  fk_country       integer,
  fk_departement   integer,
  code             varchar(62),
  dayrule          varchar(64) DEFAULT '',
  day              integer,
  month            integer,
  year             integer,
  active           integer DEFAULT 1,
  import_key       varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_c_incoterms (
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  code varchar(8) NOT NULL,
  label varchar(100),
  libelle varchar(255) NOT NULL,
  active tinyint DEFAULT 1  NOT NULL
) ENGINE=innodb;
create table llx_c_input_method
(
  rowid    integer AUTO_INCREMENT PRIMARY KEY,
  code     varchar(30),
  libelle  varchar(128),
  active   tinyint default 1  NOT NULL,
  module   varchar(32) NULL
)ENGINE=innodb;
create table llx_c_input_reason
(
  rowid		integer AUTO_INCREMENT PRIMARY KEY,
  code		varchar(30),
  label		varchar(128),
  active	tinyint default 1  NOT NULL,
  module    varchar(32) NULL
)ENGINE=innodb;
CREATE TABLE llx_c_invoice_subtype
(
	rowid		integer AUTO_INCREMENT PRIMARY KEY,
	entity		integer DEFAULT 1 NOT NULL,
	fk_country	integer NOT NULL,
	code		varchar(5) NOT NULL,
	label		varchar(200),
	active		tinyint DEFAULT 1 NOT NULL
) ENGINE=innodb;
create table llx_c_lead_status
(
  rowid       integer AUTO_INCREMENT PRIMARY KEY,
  code 		  varchar(10),
  label       varchar(128),
  position    integer,
  percent     double(5,2),
  active      tinyint DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_paiement
(
  id					integer AUTO_INCREMENT PRIMARY KEY,
  entity				integer	DEFAULT 1 NOT NULL,
  code       		varchar(6)  NOT NULL,
  libelle    		varchar(128),
  type       		smallint,
  active     		tinyint DEFAULT 1  NOT NULL,
  accountancy_code	varchar(32) NULL,
  module     		varchar(32) NULL,
  position			integer NOT NULL DEFAULT 0
)ENGINE=innodb;
create table llx_c_paper_format
(
  rowid    integer                          AUTO_INCREMENT PRIMARY KEY,
  code     varchar(16)                      NOT NULL,
  label    varchar(128)                      NOT NULL,
  width    float(6,2)                       DEFAULT 0,
  height   float(6,2)                       DEFAULT 0,
  unit     varchar(5)                       NOT NULL,
  active   tinyint DEFAULT 1                NOT NULL,
  module  varchar(32) NULL
)ENGINE=innodb;
create table llx_c_partnership_type
(
  rowid      	integer AUTO_INCREMENT PRIMARY KEY,
  entity        integer DEFAULT 1 NOT NULL,
  code          varchar(32) NOT NULL,
  label 	    varchar(128)	NOT NULL,
  keyword		varchar(128),
  active  	    tinyint DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_payment_term
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  entity				integer	DEFAULT 1 NOT NULL,
  code				varchar(16),
  sortorder			smallint,
  active				tinyint DEFAULT 1,
  libelle			varchar(255),
  libelle_facture	text,
  type_cdr			tinyint,
  nbjour				smallint,
  decalage			smallint,
  deposit_percent		varchar(63) DEFAULT NULL,
  module				varchar(32) NULL,
  position			integer NOT NULL DEFAULT 0
)ENGINE=innodb;
create table llx_c_price_expression
(
	rowid			integer AUTO_INCREMENT PRIMARY KEY,
	title			varchar(20) NOT NULL,
	expression		varchar(255) NOT NULL
)ENGINE=innodb;
create table llx_c_price_global_variable
(
	rowid					integer AUTO_INCREMENT PRIMARY KEY,
	code					varchar(20) NOT NULL,
	description		text DEFAULT NULL,
	value					double(24,8) DEFAULT 0
)ENGINE=innodb;
create table llx_c_price_global_variable_updater
(
	rowid						integer AUTO_INCREMENT PRIMARY KEY,
	type						integer NOT NULL,
	description			text DEFAULT NULL,
	parameters			text DEFAULT NULL,
	fk_variable			integer NOT NULL,
	update_interval	integer DEFAULT 0,
	next_update			integer DEFAULT 0,
	last_status			text DEFAULT NULL
)ENGINE=innodb;
CREATE TABLE llx_c_product_nature (
     rowid integer AUTO_INCREMENT PRIMARY KEY,
     code tinyint NOT NULL,
     label varchar(128),
     active tinyint DEFAULT 1  NOT NULL
) ENGINE=innodb;
CREATE TABLE llx_c_product_thirdparty_relation_type
(
    rowid  integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    code   varchar(24) NOT NULL,
    label  varchar(128),
    active tinyint default 1 NOT NULL
)ENGINE=innodb;
CREATE TABLE llx_c_productbatch_qcstatus
(
  rowid    integer            AUTO_INCREMENT PRIMARY KEY,
  entity 	 integer NOT NULL DEFAULT 1,
  code     varchar(16)        NOT NULL,
  label    varchar(128)        NOT NULL,
  active   integer  DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_propalst
(
  id              smallint    PRIMARY KEY,
  code            varchar(12) NOT NULL,
  label           varchar(128),
  sortorder       smallint DEFAULT 0,
  active          tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_prospectcontactlevel
(
  code            varchar(12) PRIMARY KEY,
  label           varchar(128),
  sortorder       smallint,
  active          smallint    DEFAULT 1 NOT NULL,
  module          varchar(32) NULL
) ENGINE=innodb;
create table llx_c_prospectlevel
(
  code            varchar(12) PRIMARY KEY,
  label           varchar(128),
  sortorder       smallint,
  active          smallint    DEFAULT 1 NOT NULL,
  module          varchar(32) NULL
) ENGINE=innodb;
create table llx_c_recruitment_origin
(
  rowid      	integer AUTO_INCREMENT PRIMARY KEY,
  code          varchar(32) NOT NULL,
  label 	    varchar(128)	NOT NULL,
  active  	    tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_regions
(
  rowid       integer AUTO_INCREMENT PRIMARY KEY,
  code_region integer NOT NULL,
  fk_pays     integer NOT NULL,
  cheflieu    varchar(50),
  tncc        integer,
  nom         varchar(100),
  active      tinyint DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_revenuestamp
(
  rowid             integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  fk_pays           integer NOT NULL,
  taux              double  NOT NULL,
  revenuestamp_type varchar(16) DEFAULT 'fixed' NOT NULL,
  note              varchar(128),
  active            tinyint DEFAULT 1 NOT NULL,
  accountancy_code_sell	varchar(32) DEFAULT NULL,
  accountancy_code_buy	varchar(32) DEFAULT NULL
)ENGINE=innodb;
create table llx_c_shipment_mode
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  entity				   integer	DEFAULT 1 NOT NULL,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  code             varchar(30) NOT NULL,
  libelle          varchar(128) NOT NULL,
  description      text,
  tracking         varchar(255) NULL,
  active           tinyint DEFAULT 0,
  module           varchar(32) NULL
)ENGINE=innodb;
create table llx_c_shipment_package_type
(
    rowid        integer  AUTO_INCREMENT PRIMARY KEY,
    label        varchar(128) NOT NULL,
    description	 varchar(255),
    active       integer DEFAULT 1 NOT NULL,
    entity       integer DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_socialnetworks
(
  rowid       integer AUTO_INCREMENT PRIMARY KEY,
  entity      integer DEFAULT 1 NOT NULL,
  code        varchar(100),
  label       varchar(150),
  url         text,
  icon        varchar(20),
  active      tinyint DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_c_stcomm
(
  id       integer      PRIMARY KEY,
  code     varchar(24)  NOT NULL,
  libelle  varchar(128),
  picto    varchar(128),
  sortorder smallint DEFAULT 0,
  active   tinyint default 1  NOT NULL
)ENGINE=innodb;
create table llx_c_stcommcontact
(
  id       integer      PRIMARY KEY,
  code     varchar(12)  NOT NULL,
  libelle  varchar(128),
  picto    varchar(128),
  active   tinyint default 1  NOT NULL
)ENGINE=innodb;
create table llx_c_ticket_category
(
  rowid			 integer AUTO_INCREMENT PRIMARY KEY,
  entity		 integer DEFAULT 1,
  code			 varchar(32) NOT NULL,
  label			 varchar(128) NOT NULL,
  public         integer DEFAULT 0,
  use_default	 integer DEFAULT 1,
  fk_parent      integer DEFAULT 0 NOT NULL,
  force_severity varchar(32) NULL,
  description	 varchar(255),
  pos			 integer DEFAULT 0 NOT NULL,
  active		 integer DEFAULT 1,
  fk_ticket_type integer NULL
)ENGINE=innodb;
create table llx_c_ticket_resolution
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  entity		integer DEFAULT 1,
  code			varchar(32)				NOT NULL,
  pos			varchar(32)				NOT NULL,
  label			varchar(128)			NOT NULL,
  active		integer DEFAULT 1,
  use_default	integer DEFAULT 1,
  description	varchar(255)
)ENGINE=innodb;
create table llx_c_ticket_severity
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  entity		integer DEFAULT 1,
  code			varchar(32)				NOT NULL,
  pos			varchar(32)				NOT NULL,
  label			varchar(128)			NOT NULL,
  color			varchar(10)				NULL,
  active		integer DEFAULT 1,
  use_default	integer DEFAULT 1,
  description	varchar(255)
)ENGINE=innodb;
create table llx_c_ticket_type
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  entity		integer DEFAULT 1,
  code			varchar(32)				NOT NULL,
  pos			varchar(32)				NOT NULL,
  label			varchar(128)			NOT NULL,
  active		integer DEFAULT 1,
  use_default	integer DEFAULT 1,
  description	varchar(255)
)ENGINE=innodb;
CREATE TABLE llx_c_transport_mode (
  rowid     integer AUTO_INCREMENT PRIMARY KEY,
  entity    integer	DEFAULT 1 NOT NULL,
  code      varchar(3) NOT NULL,
  label     varchar(255) NOT NULL,
  active    tinyint DEFAULT 1  NOT NULL
) ENGINE=innodb;
create table llx_c_tva
(
  rowid                 integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  entity                integer DEFAULT 1 NOT NULL,
  fk_pays               integer NOT NULL,
  fk_department_buyer   integer DEFAULT NULL,
  code                  varchar(10) DEFAULT '',
  type_vat              smallint NOT NULL DEFAULT 0,
  taux                  double  NOT NULL,
  localtax1             varchar(20)  NOT NULL DEFAULT '0',
  localtax1_type        varchar(10)	 NOT NULL DEFAULT '0',
  localtax2             varchar(20)  NOT NULL DEFAULT '0',
  localtax2_type        varchar(10)  NOT NULL DEFAULT '0',
  use_default           tinyint DEFAULT 0,
  recuperableonly       integer NOT NULL DEFAULT 0,
  einvoice_vatex		varchar(32),
  note                  varchar(128),
  active                tinyint DEFAULT 1 NOT NULL,
  accountancy_code_sell varchar(32) DEFAULT NULL,
  accountancy_code_buy	varchar(32) DEFAULT NULL
)ENGINE=innodb;
create table llx_c_type_contact
(
  rowid      	integer     AUTO_INCREMENT PRIMARY KEY,
  element       varchar(64) NOT NULL,
  source        varchar(8)  DEFAULT 'external' NOT NULL,
  code          varchar(32) NOT NULL,
  libelle 	    varchar(128)	NOT NULL,
  active  	    tinyint DEFAULT 1  NOT NULL,
  module        varchar(32) NULL,
  position      integer NOT NULL DEFAULT 0
)ENGINE=innodb;
create table llx_c_type_container
(
  rowid      	integer AUTO_INCREMENT PRIMARY KEY,
  code          varchar(32) NOT NULL,
  entity		integer	DEFAULT 1 NOT NULL,
  label 	    varchar(128) NOT NULL,
  module     	varchar(32) NULL,
  typecontainer varchar(10) DEFAULT 'page',
  position      integer DEFAULT 0,
  active  	    tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_type_fees
(
  id        		integer      AUTO_INCREMENT PRIMARY KEY,
  code      		varchar(12)  NOT NULL,
  label				varchar(128),
  type				integer DEFAULT 0,
  accountancy_code	varchar(32) NULL,
  active    		tinyint DEFAULT 1   NOT NULL,
  module    		varchar(32) NULL,
  position          integer NOT NULL DEFAULT 0
)ENGINE=innodb;
create table llx_c_type_resource
(
  rowid      	integer AUTO_INCREMENT PRIMARY KEY,
  code          varchar(32) NOT NULL,
  label 	    varchar(128)	NOT NULL,
  active  	    tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_c_typent
(
  id         integer      PRIMARY KEY,
  code       varchar(12)  NOT NULL,
  libelle    varchar(128),
  fk_country integer NULL,
  active     tinyint DEFAULT 1   NOT NULL,
  module     varchar(32) NULL,
  position   integer NOT NULL DEFAULT 0
)ENGINE=innodb;
create table llx_c_units(
	rowid           integer AUTO_INCREMENT PRIMARY KEY,
	code            varchar(3),
    sortorder		smallint,
	scale           integer,
	label           varchar(128),
	short_label     varchar(5),
	unit_type       varchar(10),
	active          tinyint DEFAULT 1 NOT NULL
) ENGINE=innodb;
create table llx_c_ziptown
(
  rowid				integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  code				varchar(5) DEFAULT NULL,
  fk_county			integer,
  fk_pays           integer NOT NULL DEFAULT 0,
  zip	 			varchar(10) NOT NULL,
  town				varchar(180) NOT NULL,
  town_up			varchar(180),
  active 			tinyint NOT NULL DEFAULT 1
)ENGINE=innodb;
create table llx_categorie
(
	rowid 		    integer AUTO_INCREMENT PRIMARY KEY,
	entity          integer DEFAULT 1 NOT NULL,
	fk_parent		integer DEFAULT 0 NOT NULL,
	label 		    varchar(180) NOT NULL,
	ref_ext			varchar(255),
	type	        integer DEFAULT 1 NOT NULL,
	description 	text,
    color           varchar(8),
	fk_soc          integer DEFAULT NULL,
	extraparams		varchar(255),
	date_creation	datetime,
	fk_user_creat	integer,
	fk_user_modif	integer,
	tms     		timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	visible         tinyint DEFAULT 1 NOT NULL,
	position        integer DEFAULT 0,
	import_key      varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_categorie_account
(
  fk_categorie  integer NOT NULL,
  fk_account    integer NOT NULL,
  import_key    varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_categorie_actioncomm
(
  fk_categorie integer NOT NULL,
  fk_actioncomm integer NOT NULL,
  import_key varchar(14)
) ENGINE=innodb;
create table llx_categorie_contact
(
  fk_categorie  integer NOT NULL,
  fk_socpeople  integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_fichinter
(
  fk_categorie  integer NOT NULL,
  fk_fichinter  integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_fournisseur
(
  fk_categorie  integer NOT NULL,
  fk_soc    integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_invoice
(
  fk_categorie integer NOT NULL,
  fk_invoice   integer NOT NULL,
  import_key   varchar(14)
)ENGINE=innodb;
create table llx_categorie_knowledgemanagement(
  fk_categorie  integer NOT NULL,
  fk_knowledgemanagement   integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_lang
(
  rowid          integer AUTO_INCREMENT PRIMARY KEY,
  fk_category    integer      DEFAULT 0 NOT NULL,
  lang           varchar(5)   DEFAULT 0 NOT NULL,
  label          varchar(255) NOT NULL,
  description    text
)ENGINE=innodb;
CREATE TABLE llx_categorie_lang_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_categorie_member
(
  fk_categorie  integer NOT NULL,
  fk_member     integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_mo
(
  fk_categorie integer NOT NULL,
  fk_mo        integer NOT NULL,
  import_key   varchar(14)
)ENGINE=innodb;
create table llx_categorie_order
(
  fk_categorie integer NOT NULL,
  fk_order     integer NOT NULL,
  import_key   varchar(14)
)ENGINE=innodb;
create table llx_categorie_product
(
  fk_categorie  integer NOT NULL,
  fk_product    integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_project
(
  fk_categorie  integer NOT NULL,
  fk_project    integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_project_task
(
  fk_categorie  integer NOT NULL,
  fk_project_task    integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_propal
(
  fk_categorie      integer NOT NULL,
  fk_propal integer NOT NULL,
  import_key        varchar(14)
)ENGINE=innodb;
create table llx_categorie_societe
(
  fk_categorie  integer NOT NULL,
  fk_soc    	integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_supplier_invoice
(
  fk_categorie        integer NOT NULL,
  fk_supplier_invoice integer NOT NULL,
  import_key          varchar(14)
)ENGINE=innodb;
create table llx_categorie_supplier_order
(
  fk_categorie      integer NOT NULL,
  fk_supplier_order integer NOT NULL,
  import_key        varchar(14)
)ENGINE=innodb;
create table llx_categorie_supplier_proposal
(
  fk_categorie        integer NOT NULL,
  fk_supplier_proposal integer NOT NULL,
  import_key          varchar(14)
)ENGINE=innodb;
create table llx_categorie_ticket
(
  fk_categorie  integer NOT NULL,
  fk_ticket    integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_categorie_user
(
  fk_categorie 	integer NOT NULL,
  fk_user 		integer NOT NULL,
  import_key 	varchar(14)
) ENGINE=innodb;
create table llx_categorie_warehouse
(
  fk_categorie  integer NOT NULL,
  fk_warehouse  integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_categorie_website_page
(
  fk_categorie  	integer NOT NULL,
  fk_website_page   integer NOT NULL,
  import_key    	varchar(14)
)ENGINE=innodb;
create table llx_categories_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_category_bankline
(
  lineid   integer NOT NULL,
  fk_categ integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_chargesociales
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  ref               varchar(16),
  date_ech          datetime NOT NULL,
  libelle           varchar(80) NOT NULL,
  entity            integer DEFAULT 1 NOT NULL,
  tms               timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  date_creation     datetime,
  date_valid        datetime,
  fk_user integer   DEFAULT NULL,
  fk_user_author    integer,
  fk_user_modif     integer,
  fk_user_valid     integer,
  fk_type           integer NOT NULL,
  fk_account        integer,
  fk_mode_reglement integer,
  amount            double(24,8) default 0 NOT NULL,
  paye              smallint default 0 NOT NULL,
  periode           date,
  fk_projet         integer DEFAULT NULL,
  note_private		text,
  note_public		text,
  import_key        varchar(14)
)ENGINE=innodb;
create table llx_commande
(
  rowid						integer AUTO_INCREMENT PRIMARY KEY,
  ref						varchar(30)       NOT NULL,
  entity					integer DEFAULT 1 NOT NULL,
  ref_ext					varchar(255),
  ref_client				varchar(255),
  fk_soc					integer NOT NULL,
  fk_projet					integer DEFAULT NULL,
  tms						timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  date_creation				datetime,
  date_valid				datetime,
  date_cloture				datetime,
  date_commande				date,
  fk_user_author			integer,
  fk_user_modif				integer,
  fk_user_valid				integer,
  fk_user_cloture			integer,
  source					smallint,
  fk_statut					smallint  DEFAULT 0,
  amount_ht					double(24,8) DEFAULT 0,
  remise_percent			real      DEFAULT 0,
  remise_absolue			real      DEFAULT 0,
  remise					real      DEFAULT 0,
  total_tva                 double(24,8)     DEFAULT 0,
  localtax1					double(24,8)     DEFAULT 0,
  localtax2					double(24,8)     DEFAULT 0,
  revenuestamp              double(24,8)     DEFAULT 0,
  total_ht					double(24,8)     DEFAULT 0,
  total_ttc					double(24,8)     DEFAULT 0,
  signed_status				smallint DEFAULT NULL,
  note_private				text,
  note_public				text,
  model_pdf					varchar(255),
  last_main_doc				varchar(255),
  module_source				varchar(32),
  pos_source				varchar(32),
  facture					tinyint   DEFAULT 0,
  fk_account				integer,
  fk_currency				varchar(3),
  fk_cond_reglement			integer,
  deposit_percent			varchar(63) DEFAULT NULL,
  fk_mode_reglement			integer,
  date_livraison			datetime DEFAULT NULL,
  fk_shipping_method		integer,
  fk_warehouse				integer DEFAULT NULL,
  fk_availability			integer NULL,
  fk_input_reason			integer,
  fk_delivery_address		integer,
  fk_incoterms				integer,
  location_incoterms		varchar(255),
  import_key				varchar(14),
  extraparams				varchar(255),
  fk_multicurrency			integer,
  multicurrency_code		varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_total_ht	double(24,8) DEFAULT 0,
  multicurrency_total_tva	double(24,8) DEFAULT 0,
  multicurrency_total_ttc	double(24,8) DEFAULT 0,
  ip 						varchar(250),
  user_agent				varchar(255)
)ENGINE=innodb;
create table llx_commande_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_commande_fournisseur
(
  rowid						integer AUTO_INCREMENT PRIMARY KEY,
  ref						varchar(180) NOT NULL,
  entity					integer DEFAULT 1 NOT NULL,
  ref_ext					varchar(255),
  ref_supplier				varchar(255),
  fk_soc					integer NOT NULL,
  fk_projet					integer DEFAULT 0,
  tms						timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  date_creation				datetime,
  date_valid				datetime,
  date_approve				datetime,
  date_approve2				datetime,
  date_commande				date,
  fk_user_author			integer,
  fk_user_modif				integer,
  fk_user_valid				integer,
  fk_user_approve			integer,
  fk_user_approve2			integer,
  source					smallint NOT NULL,
  fk_statut					smallint  default 0,
  billed					smallint  default 0,
  amount_ht					double(24,8)      default 0,
  remise_percent			real      default 0,
  remise					real      default 0,
  total_tva						double(24,8)      default 0,
  localtax1					double(24,8)      default 0,
  localtax2					double(24,8)      default 0,
  total_ht					double(24,8)      default 0,
  total_ttc					double(24,8)      default 0,
  note_private				text,
  note_public				text,
  model_pdf					varchar(255),
  last_main_doc				varchar(255),
  date_livraison			datetime default NULL,
  date_reception        	datetime default NULL,
  fk_account				integer,
  fk_cond_reglement			integer,
  deposit_percent			varchar(63) DEFAULT NULL,
  fk_mode_reglement			integer,
  fk_input_method			integer default 0,
  fk_incoterms				integer,
  location_incoterms		varchar(255),
  import_key				varchar(14),
  extraparams				varchar(255),
  fk_multicurrency			integer,
  multicurrency_code		varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_total_ht	double(24,8) DEFAULT 0,
  multicurrency_total_tva	double(24,8) DEFAULT 0,
  multicurrency_total_ttc	double(24,8) DEFAULT 0
)ENGINE=innodb;
create table llx_commande_fournisseur_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_commande_fournisseur_log
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datelog          datetime NOT NULL,
  fk_commande      integer NOT NULL,
  fk_statut        smallint NOT NULL,
  fk_user          integer NOT NULL,
  comment          varchar(255) NULL
)ENGINE=innodb;
create table llx_commande_fournisseurdet
(
  rowid                      integer AUTO_INCREMENT PRIMARY KEY,
  fk_commande                integer      NOT NULL,
  fk_parent_line             integer      NULL,
  fk_product                 integer,
  ref                        varchar(128),
  label                      varchar(255),
  description                text,
  vat_src_code               varchar(10)  DEFAULT '',
  tva_tx                     double(7,4)  DEFAULT 0,
  localtax1_tx               double(7,4)  DEFAULT 0,
  localtax1_type             varchar(10)  NULL,
  localtax2_tx               double(7,4)  DEFAULT 0,
  localtax2_type             varchar(10)  NULL,
  qty                        real,
  remise_percent             real         DEFAULT 0,
  remise                     real         DEFAULT 0,
  subprice                   double(24,8) DEFAULT 0,
  subprice_ttc               double(24,8) DEFAULT 0,
  total_ht                   double(24,8) DEFAULT 0,
  total_tva                  double(24,8) DEFAULT 0,
  total_localtax1            double(24,8) DEFAULT 0,
  total_localtax2            double(24,8) DEFAULT 0,
  total_ttc                  double(24,8) DEFAULT 0,
  product_type               integer      DEFAULT 0,
  date_start                 datetime     DEFAULT NULL,
  date_end                   datetime     DEFAULT NULL,
  info_bits	                 integer      DEFAULT 0,
  special_code               integer      DEFAULT 0,
  rang                       integer      DEFAULT 0,
  import_key                 varchar(14),
  fk_unit                    integer      DEFAULT NULL,
  fk_multicurrency           integer,
  multicurrency_code         varchar(3),
  multicurrency_subprice     double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc double(24,8) DEFAULT 0,
  multicurrency_total_ht     double(24,8) DEFAULT 0,
  multicurrency_total_tva    double(24,8) DEFAULT 0,
  multicurrency_total_ttc    double(24,8) DEFAULT 0,
  extraparams				 varchar(255)
)ENGINE=innodb;
create table llx_commande_fournisseurdet_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_commandedet
(
  rowid							integer AUTO_INCREMENT PRIMARY KEY,
  fk_commande					integer NOT NULL,
  fk_parent_line				integer NULL,
  fk_product					integer	 NULL,
  label							varchar(255) DEFAULT NULL,
  description					text,
  vat_src_code					varchar(10)  DEFAULT '',
  tva_tx						double(7,4),
  localtax1_tx					double(7,4)  DEFAULT 0,
  localtax1_type			 	varchar(10)  NULL,
  localtax2_tx					double(7,4)  DEFAULT 0,
  localtax2_type			 	varchar(10)	  	 NULL,
  qty							real,
  remise_percent				real         DEFAULT 0,
  remise						real         DEFAULT 0,
  fk_remise_except				integer      NULL,
  price							real,
  subprice						double(24,8) DEFAULT 0,
  subprice_ttc               	double(24,8) DEFAULT 0,
  total_ht						double(24,8) DEFAULT 0,
  total_tva						double(24,8) DEFAULT 0,
  total_localtax1				double(24,8) DEFAULT 0,
  total_localtax2				double(24,8) DEFAULT 0,
  total_ttc						double(24,8) DEFAULT 0,
  product_type					integer      DEFAULT 0,
  date_start					datetime     DEFAULT NULL,
  date_end						datetime     DEFAULT NULL,
  info_bits						integer      DEFAULT 0,
  buy_price_ht					double(24,8) DEFAULT 0,
  fk_product_fournisseur_price	integer      DEFAULT NULL,
  special_code					integer      DEFAULT 0,
  rang							integer      DEFAULT 0,
  fk_unit						integer      DEFAULT NULL,
  import_key					varchar(14),
  ref_ext                       varchar(255) DEFAULT NULL,
  fk_commandefourndet			integer DEFAULT NULL,
  fk_multicurrency				integer,
  multicurrency_code			varchar(3),
  multicurrency_subprice		double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc	double(24,8) DEFAULT 0,
  multicurrency_total_ht		double(24,8) DEFAULT 0,
  multicurrency_total_tva		double(24,8) DEFAULT 0,
  multicurrency_total_ttc		double(24,8) DEFAULT 0,
  extraparams					varchar(255)
)ENGINE=innodb;
create table llx_commandedet_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_comment (
    rowid integer AUTO_INCREMENT PRIMARY KEY,
    datec datetime  DEFAULT NULL,
    tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    description text NOT NULL,
    fk_user_author integer DEFAULT NULL,
    fk_user_modif integer DEFAULT NULL,
    fk_element integer DEFAULT NULL,
    element_type varchar(50) DEFAULT NULL,
    entity integer DEFAULT 1,
    import_key varchar(125) DEFAULT NULL
)ENGINE=innodb;
create table llx_const
(
  rowid       integer AUTO_INCREMENT PRIMARY KEY,
  name        varchar(180) NOT NULL,
  entity      integer DEFAULT 1 NOT NULL,
  value       text NOT NULL,
  type        varchar(64) DEFAULT 'string',
  visible     tinyint DEFAULT 1 NOT NULL,
  note        text,
  tms         timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=innodb;
create table llx_contrat
(
  rowid						integer AUTO_INCREMENT PRIMARY KEY,
  ref						varchar(255),
  ref_customer				varchar(255),
  ref_supplier				varchar(255),
  ref_ext					varchar(255),
  entity					integer DEFAULT 1 NOT NULL,
  tms						timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec						datetime,
  date_contrat				datetime,
  statut					smallint DEFAULT 0,
  fin_validite				datetime,
  date_cloture				datetime,
  fk_soc					integer NOT NULL,
  fk_projet					integer,
  fk_commercial_signature	integer,
  fk_commercial_suivi 		integer,
  fk_user_author			integer NOT NULL default 0,
  fk_user_modif				integer,
  fk_user_cloture			integer,
  total_tva                 double(24,8)     DEFAULT 0,
  localtax1					double(24,8)     DEFAULT 0,
  localtax2					double(24,8)     DEFAULT 0,
  revenuestamp              double(24,8)     DEFAULT 0,
  total_ht					double(24,8)     DEFAULT 0,
  total_ttc					double(24,8)     DEFAULT 0,
  denormalized_lower_planned_end_date datetime,
  signed_status        		smallint DEFAULT NULL,
  online_sign_ip			varchar(48),
  online_sign_name			varchar(64),
  note_private				text,
  note_public				text,
  model_pdf					varchar(255),
  last_main_doc			    varchar(255),
  import_key				varchar(14),
  extraparams				varchar(255)
)ENGINE=innodb;
create table llx_contrat_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_contratdet
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  tms                   timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_contrat            integer       NOT NULL,
  fk_product            integer       NULL,
  statut                smallint      DEFAULT 0,
  label                 text,
  description           text,
  fk_remise_except		integer       NULL,
  date_commande         datetime,
  date_ouverture_prevue datetime,
  date_ouverture        datetime,
  date_fin_validite     datetime,
  date_cloture          datetime,
  vat_src_code			varchar(10)   DEFAULT '',
  tva_tx                double(7,4)   DEFAULT 0,
  localtax1_tx		    double(7,4)   DEFAULT 0,
  localtax1_type		varchar(10)	  	 NULL,
  localtax2_tx		    double(7,4)   DEFAULT 0,
  localtax2_type		varchar(10)	  	 NULL,
  qty                   real          NOT NULL,
  remise_percent        real          DEFAULT 0,
  subprice              double(24,8)  DEFAULT 0,
  subprice_ttc         	double(24,8)  DEFAULT 0,
  total_ht              double(24,8)  DEFAULT 0,
  total_tva             double(24,8)  DEFAULT 0,
  total_localtax1       double(24,8)  DEFAULT 0,
  total_localtax2       double(24,8)  DEFAULT 0,
  total_ttc             double(24,8)  DEFAULT 0,
  product_type			integer       DEFAULT 1,
  info_bits		        integer DEFAULT 0,
  rang					integer DEFAULT 0,
  buy_price_ht          double(24,8)  DEFAULT NULL,
  fk_product_fournisseur_price integer DEFAULT NULL,
  fk_user_author        integer       NOT NULL DEFAULT 0,
  fk_user_ouverture     integer,
  fk_user_cloture       integer,
  commentaire           text,
  fk_unit               integer       DEFAULT NULL,
  fk_multicurrency		integer,
  multicurrency_code			varchar(3),
  multicurrency_subprice		double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc	double(24,8) DEFAULT 0,
  multicurrency_total_ht		double(24,8) DEFAULT 0,
  multicurrency_total_tva	double(24,8) DEFAULT 0,
  multicurrency_total_ttc	double(24,8) DEFAULT 0,
  extraparams				 varchar(255)
)ENGINE=innodb;
create table llx_contratdet_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_cronjob
(
	rowid 			integer AUTO_INCREMENT PRIMARY KEY,
	tms 			timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	datec 			datetime,
	jobtype			varchar(10) NOT NULL,
  	label 			varchar(255) NOT NULL,
	command			varchar(255),
  	classesname 	varchar(255),
  	objectname		varchar(255),
  	methodename		varchar(255),
  	params 			text,
	md5params 		varchar(32),
  	module_name 	varchar(255),
  	priority 		integer DEFAULT 0,
  	datelastrun 	datetime,
  	datenextrun 	datetime,
  	datestart		datetime,
  	dateend			datetime,
  	datelastresult  datetime,
  	lastresult      text,
  	lastoutput      text,
  	unitfrequency	varchar(255) NOT NULL DEFAULT '3600',
  	frequency 		integer NOT NULL DEFAULT 0,
    maxrun          integer NOT NULL DEFAULT 0,
	nbrun			integer,
    autodelete      integer DEFAULT 0,
  	status 			integer NOT NULL DEFAULT 1,
  	processing 		integer NOT NULL DEFAULT 0,
  	pid             integer,
  	test		    varchar(255) DEFAULT '1',
  	fk_user_author 	integer DEFAULT NULL,
  	fk_user_mod 	integer DEFAULT NULL,
    fk_mailing      integer DEFAULT NULL,
	note 			text,
	libname			varchar(255),
	email_alert		varchar(128),
	entity			integer DEFAULT 0
)ENGINE=innodb;
create table llx_default_values
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  entity          integer DEFAULT 1 NOT NULL,
  type			  varchar(10),
  user_id         integer DEFAULT 0 NOT NULL,
  page            varchar(255),
  param           varchar(255),
  value		      varchar(128)
)ENGINE=innodb;
create table llx_delivery
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  tms                   timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  ref                   varchar(30)  NOT NULL,
  entity                integer DEFAULT 1 NOT NULL,
  fk_soc                integer      NOT NULL,
  ref_ext               varchar(255),
  ref_customer          varchar(255),
  date_creation         datetime,
  fk_user_author        integer,
  date_valid            datetime,
  fk_user_valid         integer,
  date_delivery 	    datetime		DEFAULT NULL,
  fk_address  			integer,
  fk_statut             smallint     DEFAULT 0,
  total_ht              double(24,8) DEFAULT 0,
  note_private          text,
  note_public           text,
  model_pdf             varchar(255),
  last_main_doc			varchar(255),
  fk_incoterms          integer,
  location_incoterms    varchar(255),
  import_key        varchar(14),
  extraparams		varchar(255)
)ENGINE=innodb;
create table llx_delivery_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_deliverydet
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  fk_delivery       integer,
  fk_origin_line    integer,
  fk_product        integer,
  description       text,
  qty               real,
  subprice          double(24,8) DEFAULT 0,
  subprice_ttc      double(24,8) DEFAULT 0,
  total_ht          double(24,8) DEFAULT 0,
  rang              integer      DEFAULT 0,
  extraparams		varchar(255)
)ENGINE=innodb;
create table llx_deliverydet_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_deplacement
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  ref				varchar(30) DEFAULT NULL,
  entity			integer DEFAULT 1 NOT NULL,
  datec				datetime NOT NULL,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  dated				datetime,
  fk_user			integer NOT NULL,
  fk_user_author	integer,
  fk_user_modif		integer,
  type				varchar(12) NOT NULL,
  fk_statut         integer DEFAULT 1 NOT NULL,
  km				real,
  fk_soc			integer,
  fk_projet         integer DEFAULT 0,
  note_private	    text,
  note_public       text,
  extraparams		varchar(255)
)ENGINE=innodb;
create table llx_document_model
(
  	rowid			integer AUTO_INCREMENT PRIMARY KEY,
  	nom				varchar(50),
  	entity			integer DEFAULT 1 NOT NULL,
  	type			varchar(64) NOT NULL,
  	libelle			varchar(255),
  	tms 			timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  	description		text
)ENGINE=innodb;
create table llx_don
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  ref             varchar(30) DEFAULT NULL,
  entity          integer DEFAULT 1 NOT NULL,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_statut       smallint NOT NULL DEFAULT 0,
  datedon         datetime,
  amount          double(24,8) DEFAULT 0,
  fk_payment      integer,
  paid            smallint default 0 NOT NULL,
  fk_soc      	  integer NULL,
  firstname       varchar(50),
  lastname        varchar(50),
  societe         varchar(50),
  address         text,
  zip             varchar(30),
  town            varchar(50),
  country         varchar(50),
  fk_country      integer NOT NULL,
  email           varchar(255),
  phone           varchar(24),
  phone_mobile    varchar(24),
  public          smallint DEFAULT 1 NOT NULL,
  fk_projet       integer NULL,
  datec           datetime,
  fk_user_author  integer NOT NULL,
  fk_user_modif   integer,
  date_valid      datetime,
  fk_user_valid   integer NULL,
  note_private    text,
  note_public     text,
  model_pdf       varchar(255),
  import_key      varchar(14),
  extraparams	    varchar(255),
  ip              varchar(250)
)ENGINE=innodb;
create table llx_don_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_ecm_directories
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  label				varchar(64) NOT NULL,
  entity			integer DEFAULT 1 NOT NULL,
  fk_parent			integer,
  description		varchar(255) NOT NULL,
  cachenbofdoc		integer NOT NULL DEFAULT 0,
  fullpath    		varchar(750),
  extraparams		varchar(255),
  date_c			datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_c			integer,
  fk_user_m			integer,
  note_private		text,
  note_public		text,
  acl				text
) ENGINE=innodb;
CREATE TABLE llx_ecm_directories_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_ecm_files
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  ref				varchar(128),
  label				varchar(128) NOT NULL,
  share				varchar(128) NULL,
  share_pass		varchar(32) NULL,
  entity			integer DEFAULT 1 NOT NULL,
  filepath    		varchar(255) NOT NULL,
  filename          varchar(255) NOT NULL,
  src_object_type   varchar(64),
  src_object_id     integer,
  agenda_id         integer,
  fullpath_orig		varchar(750),
  description		varchar(255),
  keywords          varchar(750),
  content			text,
  cover             varchar(32),
  position          integer,
  gen_or_uploaded   varchar(12),
  extraparams		varchar(255),
  date_c			datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_c			integer,
  fk_user_m			integer,
  note_private		text,
  note_public		text,
  acl				text,
  geolat            double(24,8)   DEFAULT NULL,
  geolong           double(24,8)   DEFAULT NULL,
  geopoint          point DEFAULT NULL,
  georesultcode     varchar(16)
) ENGINE=innodb;
CREATE TABLE llx_ecm_files_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_element_categorie
(
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  fk_categorie  integer NOT NULL,
  fk_element  integer NOT NULL,
  import_key    varchar(14)
)ENGINE=innodb;
create table llx_element_contact
(
  rowid               integer AUTO_INCREMENT PRIMARY KEY,
  datecreate          datetime NULL,
  statut              smallint DEFAULT 5,
  element_id		  int NOT NULL,
  mandatory_signature tinyint,
  fk_c_type_contact	  int NOT NULL,
  fk_socpeople        integer NOT NULL
)ENGINE=innodb;
create table llx_element_element
(
  rowid           	integer AUTO_INCREMENT PRIMARY KEY,
  fk_source			integer NOT NULL,
  sourcetype		varchar(64) NOT NULL,
  fk_target			integer NOT NULL,
  targettype		varchar(64) NOT NULL,
  relationtype		varchar(64) DEFAULT NULL
) ENGINE=innodb;
CREATE TABLE llx_element_resources
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  element_id	  integer,
  element_type    varchar(64),
  resource_id     integer,
  resource_type	  varchar(64),
  busy			  integer,
  mandatory		  integer,
  duree				real,
  fk_user_create  integer,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
CREATE TABLE llx_element_time(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    ref_ext varchar(32),
    fk_element integer NOT NULL,
    elementtype varchar(32) NOT NULL,
    element_date date,
    element_datehour datetime,
    element_date_withhour integer,
    element_duration double,
    fk_product integer,
    fk_user integer,
    thm double(24,8),
	invoice_id integer DEFAULT NULL,
	invoice_line_id integer DEFAULT NULL,
	intervention_id integer DEFAULT NULL,
	intervention_line_id integer DEFAULT NULL,
	import_key varchar(14),
	datec datetime,
    tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	note text
) ENGINE=innodb;
CREATE TABLE llx_emailcollector_emailcollector(
        rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
        entity integer DEFAULT 1 NOT NULL,
        ref varchar(128) NOT NULL,
        label varchar(255),
        description text,
        host varchar(255),
        port varchar(10) DEFAULT '993',
        hostcharset varchar(16) DEFAULT 'UTF-8',
        imap_encryption varchar(16) DEFAULT 'ssl',
        norsh integer DEFAULT 0,
        login varchar(128),
        acces_type integer DEFAULT 0,
        oauth_service varchar(128),
        password varchar(128),
        source_directory varchar(255) DEFAULT 'Inbox' NOT NULL,
        target_directory varchar(255),
        maxemailpercollect integer DEFAULT 100,
        datelastresult datetime,
        codelastresult varchar(16),
        lastresult text,
        datelastok datetime,
        note_public text,
        note_private text,
        date_creation datetime NOT NULL,
        tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
        fk_user_creat integer NOT NULL,
        fk_user_modif integer,
        position INTEGER NOT NULL DEFAULT 0,
        import_key varchar(14),
        status integer NOT NULL
) ENGINE=innodb;
CREATE TABLE llx_emailcollector_emailcollectoraction(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	fk_emailcollector INTEGER NOT NULL,
	type varchar(128) NOT NULL,
	actionparam text NULL,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	position integer DEFAULT 0,
	import_key varchar(14),
	status integer NOT NULL
) ENGINE=innodb;
CREATE TABLE llx_emailcollector_emailcollectorfilter(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	fk_emailcollector INTEGER NOT NULL,
	type varchar(128) NOT NULL,
	rulevalue varchar(128) NULL,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	import_key varchar(14),
	status integer NOT NULL
) ENGINE=innodb;
create table llx_entrepot
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  ref             varchar(255) NOT NULL,
  datec           datetime,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  entity          integer DEFAULT 1 NOT NULL,
  fk_project	  integer DEFAULT NULL,
  description     text,
  lieu            varchar(64),
  address         varchar(255),
  zip             varchar(10),
  town            varchar(50),
  fk_departement  integer,
  fk_pays         integer DEFAULT 0,
  phone           varchar(30),
  fax             varchar(30),
  barcode         varchar(180) DEFAULT NULL,
  fk_barcode_type integer      DEFAULT NULL,
  warehouse_usage integer DEFAULT 1,
  statut          tinyint DEFAULT 1,
  fk_user_author  integer,
  model_pdf       varchar(255),
  import_key	  varchar(14),
  fk_parent       integer DEFAULT 0
)ENGINE=innodb;
create table llx_entrepot_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_establishment (
  rowid 			integer NOT NULL auto_increment PRIMARY KEY,
  entity 			integer NOT NULL DEFAULT 1,
  ref				varchar(30),
  label       		varchar(255) NOT NULL,
  name				varchar(128),
  address           varchar(255),
  zip               varchar(25),
  town              varchar(50),
  fk_state          integer DEFAULT 0,
  fk_country        integer DEFAULT 0,
  profid1			varchar(20),
  profid2			varchar(20),
  profid3			varchar(20),
  phone				varchar(30),
  fk_user_author 	integer NOT NULL,
  fk_user_mod		integer,
  datec				datetime NOT NULL,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL,
  status            tinyint DEFAULT 1
) ENGINE=innodb;
CREATE TABLE llx_eventorganization_conferenceorboothattendee(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	ref varchar(128) NOT NULL,
	fk_soc integer,
	fk_actioncomm integer,
	fk_project integer NOT NULL,
	fk_invoice integer NULL,
	email varchar(128),
    email_company varchar(128),
	firstname varchar(100),
	lastname varchar(100),
	date_subscription datetime,
	amount double DEFAULT NULL,
	note_public text,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer,
	fk_user_modif integer,
	last_main_doc varchar(255),
	ip varchar(250),
	import_key varchar(14),
	model_pdf varchar(255),
	status smallint NOT NULL,
	fk_replacement integer NULL
) ENGINE=innodb;
create table llx_eventorganization_conferenceorboothattendee_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_events
(
  rowid          integer AUTO_INCREMENT PRIMARY KEY,
  tms            timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  type           varchar(32)  NOT NULL,
  entity         integer DEFAULT 1 NOT NULL,
  prefix_session varchar(255) NULL,
  dateevent      datetime,
  fk_user        integer,
  description    varchar(250) NOT NULL,
  ip             varchar(250) NOT NULL,
  user_agent     varchar(255) NULL,
  fk_object      integer NULL,
  authentication_method varchar(64) NULL,
  fk_oauth_token integer NULL
) ENGINE=innodb;
create table llx_expedition
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  tms                   timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  ref                   varchar(30)        NOT NULL,
  entity                integer  DEFAULT 1 NOT NULL,
  fk_soc                integer            NOT NULL,
  fk_projet  		    integer  DEFAULT NULL,
  ref_ext               varchar(255),
  ref_customer          varchar(255),
  date_creation         datetime,
  fk_user_author        integer,
  fk_user_modif         integer,
  date_valid            datetime,
  fk_user_valid         integer,
  date_delivery			datetime	DEFAULT NULL,
  date_expedition       datetime,
  fk_address  			integer		DEFAULT NULL,
  fk_shipping_method    integer,
  tracking_number       varchar(50),
  fk_statut             smallint	DEFAULT 0,
  billed                smallint    DEFAULT 0,
  height                float,
  width                 float,
  size_units            integer,
  size                  float,
  weight_units          integer,
  weight                float,
  signed_status         smallint DEFAULT NULL,
  online_sign_ip		varchar(48),
  online_sign_name		varchar(64),
  note_private          text,
  note_public           text,
  model_pdf             varchar(255),
  last_main_doc			varchar(255),
  fk_incoterms          integer,
  location_incoterms    varchar(255),
  import_key			varchar(14),
  extraparams			varchar(255)
)ENGINE=innodb;
create table llx_expedition_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_expedition_package
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  fk_expedition     integer NOT NULL,
  description       varchar(255),
  value             double(24,8)     DEFAULT 0,
  fk_package_type    integer,
  height            float,
  width             float,
  size              float,
  size_units        integer,
  weight            float,
  weight_units      integer,
  dangerous_goods   varchar(20) DEFAULT '0',
  tail_lift         smallint          DEFAULT 0,
  rang              integer  DEFAULT 0
)ENGINE=innodb;
create table llx_expeditiondet
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  fk_expedition     integer NOT NULL,
  fk_element        integer,
  fk_elementdet     integer,
  element_type   	varchar(50) DEFAULT 'commande' NOT NULL,
  fk_product        integer,
  fk_parent         integer,
  qty               real,
  fk_unit           integer,
  fk_entrepot       integer,
  description		text,
  rang              integer  DEFAULT 0,
  extraparams		varchar(255)
)ENGINE=innodb;
CREATE TABLE llx_expeditiondet_batch (
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  fk_expeditiondet int NOT NULL,
  eatby date DEFAULT NULL,
  sellby date DEFAULT NULL,
  batch varchar(128) DEFAULT NULL,
  qty double NOT NULL DEFAULT '0',
  fk_origin_stock integer NOT NULL,
  fk_warehouse integer DEFAULT NULL
) ENGINE=innodb;
create table llx_expeditiondet_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_expensereport (
  rowid integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  ref        		varchar(50) NOT NULL,
  entity 			integer DEFAULT 1 NOT NULL,
  ref_number_int 	integer DEFAULT NULL,
  ref_ext 			integer,
  total_ht 			double(24,8) DEFAULT 0,
  total_tva 		double(24,8) DEFAULT 0,
  localtax1			double(24,8) DEFAULT 0,
  localtax2			double(24,8) DEFAULT 0,
  total_ttc 		double(24,8) DEFAULT 0,
  date_debut 		date NOT NULL,
  date_fin 			date NOT NULL,
  date_create 		datetime NOT NULL,
  date_valid 		datetime,
  date_approve		datetime,
  date_refuse 		datetime,
  date_cancel 		datetime,
  tms 		 		timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_author 	integer NOT NULL,
  fk_user_creat 	integer DEFAULT NULL,
  fk_user_modif 	integer DEFAULT NULL,
  fk_user_valid 	integer DEFAULT NULL,
  fk_user_validator integer DEFAULT NULL,
  fk_user_approve   integer DEFAULT NULL,
  fk_user_refuse 	integer DEFAULT NULL,
  fk_user_cancel 	integer DEFAULT NULL,
  fk_statut			integer NOT NULL,
  fk_c_paiement 	integer DEFAULT NULL,
  paid              smallint default 0 NOT NULL,
  note_public		text,
  note_private 		text,
  detail_refuse 	varchar(255) DEFAULT NULL,
  detail_cancel 	varchar(255) DEFAULT NULL,
  integration_compta integer DEFAULT NULL,
  fk_bank_account 	integer DEFAULT NULL,
  model_pdf 		varchar(255) DEFAULT NULL,
  last_main_doc		varchar(255),
  fk_multicurrency        integer,
  multicurrency_code      varchar(3),
  multicurrency_tx        double(24,8) DEFAULT 1,
  multicurrency_total_ht  double(24,8) DEFAULT 0,
  multicurrency_total_tva double(24,8) DEFAULT 0,
  multicurrency_total_ttc double(24,8) DEFAULT 0,
  import_key			varchar(14),
  extraparams			varchar(255)
) ENGINE=innodb;
CREATE TABLE llx_expensereport_det
(
   rowid integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
   fk_expensereport integer NOT NULL,
   docnumber varchar(128),
   fk_c_type_fees integer NOT NULL,
   fk_c_exp_tax_cat integer,
   fk_projet integer,
   comments text NOT NULL,
   product_type integer DEFAULT -1,
   qty real NOT NULL,
   subprice						double(24,8) DEFAULT 0 NOT NULL,
   subprice_ttc         		double(24,8) DEFAULT 0,
   value_unit                   double(24,8) NOT NULL,
   remise_percent real,
   vat_src_code					varchar(10)  DEFAULT '',
   tva_tx						double(7,4),
   localtax1_tx               	double(7,4)  DEFAULT 0,
   localtax1_type			 	varchar(10)	  	 NULL,
   localtax2_tx               	double(7,4)  DEFAULT 0,
   localtax2_type			 	varchar(10)	  	 NULL,
   total_ht double(24,8) DEFAULT 0 NOT NULL,
   total_tva double(24,8) DEFAULT 0 NOT NULL,
   total_localtax1				double(24,8)  	DEFAULT 0,
   total_localtax2				double(24,8)	DEFAULT 0,
   total_ttc double(24,8) DEFAULT 0 NOT NULL,
   date date NOT NULL,
   info_bits					integer DEFAULT 0,
   special_code					integer DEFAULT 0,
   fk_multicurrency             integer,
   multicurrency_code           varchar(3),
   multicurrency_subprice       double(24,8) DEFAULT 0,
   multicurrency_subprice_ttc   double(24,8) DEFAULT 0,
   multicurrency_total_ht       double(24,8) DEFAULT 0,
   multicurrency_total_tva      double(24,8) DEFAULT 0,
   multicurrency_total_ttc      double(24,8) DEFAULT 0,
   fk_facture					integer DEFAULT 0,
   fk_ecm_files        integer DEFAULT NULL,
   tcheck_file			integer DEFAULT NULL,
   fk_code_ventilation			integer DEFAULT 0,
   rang							integer DEFAULT 0,
   import_key					varchar(14),
   rule_warning_message text
) ENGINE=innodb;
CREATE TABLE llx_expensereport_det_extrafields
(
	rowid                     integer AUTO_INCREMENT PRIMARY KEY,
	tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_object                 integer NOT NULL,
	import_key                varchar(14)
) ENGINE=innodb;
create table llx_expensereport_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE IF NOT EXISTS llx_expensereport_ik (
    rowid           integer  AUTO_INCREMENT PRIMARY KEY,
    datec           datetime  DEFAULT NULL,
    tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_c_exp_tax_cat integer DEFAULT 0 NOT NULL,
    fk_range        integer DEFAULT 0 NOT NULL,
    coef            double DEFAULT 0 NOT NULL,
    ikoffset        double DEFAULT 0 NOT NULL,
    active          integer DEFAULT 1
)ENGINE=innodb;
CREATE TABLE llx_expensereport_rules (
  rowid						integer AUTO_INCREMENT PRIMARY KEY,
  datec						datetime  DEFAULT NULL,
  tms						timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  dates						datetime NOT NULL,
  datee						datetime NOT NULL,
  amount					double(24,8) NOT NULL,
  restrictive				tinyint NOT NULL,
  fk_user					integer DEFAULT NULL,
  fk_usergroup				integer DEFAULT NULL,
  fk_c_type_fees			integer NOT NULL,
  code_expense_rules_type	varchar(50) NOT NULL,
  is_for_all				tinyint DEFAULT 0,
  entity					integer DEFAULT 1
) ENGINE=innodb;
create table llx_export_model
(
  	rowid		integer AUTO_INCREMENT PRIMARY KEY,
  	entity      integer DEFAULT 0,
	fk_user		integer DEFAULT 0 NOT NULL,
  	label		varchar(50) NOT NULL,
  	type		varchar(64) NOT NULL,
  	field		text NOT NULL,
  	filter		text
)ENGINE=innodb;
create table llx_extrafields
(
	rowid           integer AUTO_INCREMENT PRIMARY KEY,
	name            varchar(64) NOT NULL,
	entity          integer DEFAULT 1 NOT NULL,
    elementtype     varchar(64) NOT NULL DEFAULT 'member',
	label           varchar(255) NOT NULL,
	type            varchar(8),
	size            varchar(8) DEFAULT NULL,
	fieldcomputed   text,
	fielddefault    text,
	fieldunique     integer DEFAULT 0,
	fieldrequired   integer DEFAULT 0,
	perms			varchar(255),
	enabled         varchar(255),
	module          varchar(64),
	pos             integer DEFAULT 0,
	alwayseditable  integer DEFAULT 0,
	emptyonclone    integer DEFAULT 0,
	param			text,
	list			varchar(255) DEFAULT '1',
	printable		integer DEFAULT 0,
    showintooltip	integer DEFAULT 0,
    totalizable     boolean DEFAULT FALSE,
	langs			varchar(64),
	help            text,
	aiprompt		text,
	css             varchar(128),
	cssview         varchar(128),
	csslist         varchar(128),
    personal_data	tinyint DEFAULT 0,
	fk_user_author	integer,
	fk_user_modif	integer,
	datec			datetime,
	tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
create table llx_facture
(
  rowid					integer AUTO_INCREMENT PRIMARY KEY,
  ref					varchar(30)        NOT NULL,
  entity				integer  DEFAULT 1 NOT NULL,
  ref_ext				varchar(255),
  ref_client			varchar(255),
  type					smallint DEFAULT 0 NOT NULL,
  subtype				smallint DEFAULT NULL,
  fk_soc				integer            NOT NULL,
  datec					datetime,
  datef					date,
  date_pointoftax		date DEFAULT NULL,
  date_valid			date,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  date_closing			datetime,
  paye					smallint DEFAULT 0 NOT NULL,
  remise_percent		real     DEFAULT 0,
  remise_absolue		real     DEFAULT 0,
  remise				real     DEFAULT 0,
  close_code			varchar(16),
  close_missing_amount	double(24,8),
  close_note			varchar(128),
  total_tva				double(24,8)     DEFAULT 0,
  localtax1				double(24,8)     DEFAULT 0,
  localtax2				double(24,8)     DEFAULT 0,
  revenuestamp          double(24,8)     DEFAULT 0,
  total_ht				double(24,8)     DEFAULT 0,
  total_ttc				double(24,8)     DEFAULT 0,
  fk_statut				smallint DEFAULT 0 NOT NULL,
  fk_user_author		integer,
  fk_user_modif         integer,
  fk_user_valid			integer,
  fk_user_closing		integer,
  module_source			varchar(32),
  pos_source			varchar(32),
  pos_print_counter     integer DEFAULT 0,
  email_sent_counter    integer DEFAULT 0,
  fk_fac_rec_source		integer,
  fk_facture_source		integer,
  fk_projet				integer DEFAULT NULL,
  increment				varchar(10),
  fk_account			integer,
  fk_currency			varchar(3),
  fk_cond_reglement		integer  DEFAULT 1 NOT NULL,
  fk_mode_reglement		integer,
  date_lim_reglement	date,
  payment_reference     varchar(25),
  fk_thirdparty_rib_id	integer NULL,
  dispute_status		integer DEFAULT 0,
  note_private			text,
  note_public			text,
  model_pdf				varchar(255),
  last_main_doc			varchar(255),
  fk_input_reason       integer DEFAULT NULL,
  fk_incoterms          integer,
  location_incoterms    varchar(255),
  fk_transport_mode     integer,
  prorata_discount		real DEFAULT NULL,
  situation_cycle_ref integer,
  situation_counter   smallint,
  situation_final     smallint,
  retained_warranty							real DEFAULT NULL,
  retained_warranty_amount 					double(24,8) DEFAULT NULL,
  retained_warranty_date_limit				date DEFAULT NULL,
  retained_warranty_fk_cond_reglement		integer  DEFAULT NULL,
  import_key			varchar(14),
  extraparams			varchar(255),
  is_also_delivery_note tinyint DEFAULT 0 NOT NULL,
  fk_multicurrency		integer,
  multicurrency_code		varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_total_ht	double(24,8) DEFAULT 0,
  multicurrency_total_tva	double(24,8) DEFAULT 0,
  multicurrency_total_ttc	double(24,8) DEFAULT 0,
  ip  varchar(250)
)ENGINE=innodb;
create table llx_facture_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_facture_fourn
(
  rowid					integer AUTO_INCREMENT PRIMARY KEY,
  ref					varchar(180) NOT NULL,
  ref_supplier			varchar(180) NOT NULL,
  entity				integer  DEFAULT 1 NOT NULL,
  ref_ext				varchar(255),
  type					smallint DEFAULT 0 NOT NULL,
  subtype				smallint DEFAULT NULL,
  fk_soc				integer NOT NULL,
  datec					datetime,
  datef					date,
  date_pointoftax		date DEFAULT NULL,
  date_valid			date,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  date_closing			datetime,
  libelle				varchar(255),
  paye					smallint         DEFAULT 0 NOT NULL,
  amount				double(24,8)     DEFAULT 0 NOT NULL,
  remise				double(24,8)     DEFAULT 0,
  close_code			varchar(16),
  close_missing_amount	double(24,8),
  close_note			varchar(128),
  vat_reverse_charge    tinyint          DEFAULT 0,
  tva					double(24,8)     DEFAULT 0,
  total_tva				double(24,8)     DEFAULT 0,
  localtax1				double(24,8)     DEFAULT 0,
  localtax2				double(24,8)     DEFAULT 0,
  revenuestamp          double(24,8)     DEFAULT 0,
  total_ht				double(24,8)     DEFAULT 0,
  total_ttc				double(24,8)     DEFAULT 0,
  fk_statut				smallint DEFAULT 0 NOT NULL,
  fk_user_author		integer,
  fk_user_modif         integer,
  fk_user_valid			integer,
  fk_user_closing		integer,
  fk_fac_rec_source		integer,
  fk_facture_source		integer,
  fk_projet				integer,
  fk_account            integer,
  fk_cond_reglement		integer,
  fk_mode_reglement		integer,
  date_lim_reglement 	date,
  payment_reference     varchar(25),
  fk_thirdparty_rib_id	integer NULL,
  dispute_status		integer DEFAULT 0,
  note_private			text,
  note_public			text,
  fk_incoterms          integer,
  location_incoterms    varchar(255),
  fk_transport_mode     integer,
  model_pdf				varchar(255),
  last_main_doc			varchar(255),
  import_key			varchar(14),
  extraparams			varchar(255),
  fk_multicurrency		integer,
  multicurrency_code			varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_total_ht		double(24,8) DEFAULT 0,
  multicurrency_total_tva	double(24,8) DEFAULT 0,
  multicurrency_total_ttc	double(24,8) DEFAULT 0
)ENGINE=innodb;
create table llx_facture_fourn_det
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  fk_facture_fourn  integer NOT NULL,
  fk_parent_line    integer NULL,
  fk_product        integer NULL,
  ref               varchar(128),
  label             varchar(255),
  description       text,
  pu_ht             double(24,8),
  pu_ttc            double(24,8),
  qty               real,
  remise_percent	real       DEFAULT 0,
  fk_remise_except	integer    NULL,
  vat_src_code		varchar(10)  DEFAULT '',
  tva_tx            double(7,4),
  localtax1_tx      double(7,4)  DEFAULT 0,
  localtax1_type	varchar(10)	  NULL,
  localtax2_tx      double(7,4)  DEFAULT 0,
  localtax2_type	varchar(10)	  NULL,
  total_ht          double(24,8),
  tva               double(24,8),
  total_localtax1   double(24,8) DEFAULT 0,
  total_localtax2   double(24,8) DEFAULT 0,
  total_ttc         double(24,8),
  product_type	    integer      DEFAULT 0,
  date_start        datetime   DEFAULT NULL,
  date_end          datetime   DEFAULT NULL,
  info_bits						integer    DEFAULT 0,
  fk_code_ventilation integer DEFAULT 0 NOT NULL,
  special_code				 integer      DEFAULT 0,
  rang						 integer      DEFAULT 0,
  import_key        varchar(14),
  fk_unit         integer    DEFAULT NULL,
  fk_multicurrency		integer,
  multicurrency_code			varchar(3),
  multicurrency_subprice		double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc	double(24,8) DEFAULT 0,
  multicurrency_total_ht		double(24,8) DEFAULT 0,
  multicurrency_total_tva	double(24,8) DEFAULT 0,
  multicurrency_total_ttc	double(24,8) DEFAULT 0,
  extraparams				 varchar(255)
)ENGINE=innodb;
create table llx_facture_fourn_det_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_facture_fourn_det_rec
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  fk_facture_fourn		integer NOT NULL,
  fk_parent_line	integer NULL,
  fk_product		integer NULL,
  ref               varchar(50),
  label				varchar(255) DEFAULT NULL,
  description		text,
  pu_ht             double(24,8),
  pu_ttc            double(24,8),
  qty               real,
  remise_percent	real       DEFAULT 0,
  fk_remise_except	integer    NULL,
  vat_src_code					varchar(10)  DEFAULT '',
  tva_tx			double(7,4),
  localtax1_tx      double(7,4) DEFAULT 0,
  localtax1_type	varchar(10) NULL,
  localtax2_tx      double(7,4) DEFAULT 0,
  localtax2_type	varchar(10)	 NULL,
  total_ht			double(24,8),
  total_tva			double(24,8),
  total_localtax1	double(24,8) DEFAULT 0,
  total_localtax2	double(24,8) DEFAULT 0,
  total_ttc			double(24,8),
  product_type		integer DEFAULT 0,
  date_start        integer DEFAULT NULL,
  date_end          integer DEFAULT NULL,
  info_bits			integer DEFAULT 0,
  special_code		integer UNSIGNED DEFAULT 0,
  rang				integer DEFAULT 0,
  fk_unit           integer    DEFAULT NULL,
  import_key		varchar(14),
  fk_user_author	integer,
  fk_user_modif     integer,
  fk_multicurrency          integer,
  multicurrency_code        varchar(3),
  multicurrency_subprice     double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc double(24,8) DEFAULT 0,
  multicurrency_total_ht    double(24,8) DEFAULT 0,
  multicurrency_total_tva   double(24,8) DEFAULT 0,
  multicurrency_total_ttc   double(24,8) DEFAULT 0,
  extraparams				 varchar(255)
)ENGINE=innodb;
create table llx_facture_fourn_det_rec_extrafields
(
    rowid            integer AUTO_INCREMENT PRIMARY KEY,
    tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object        integer NOT NULL,
    import_key       varchar(14)
)ENGINE=innodb;
create table llx_facture_fourn_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_facture_fourn_rec
(
    rowid                       integer AUTO_INCREMENT PRIMARY KEY,
    titre                       varchar(200)        NOT NULL,
    ref_supplier			    varchar(180) NOT NULL,
    entity                      integer   DEFAULT 1 NOT NULL,
    subtype				        smallint DEFAULT NULL,
    fk_soc                      integer             NOT NULL,
    datec                       datetime,
    tms                         timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    suspended                   integer   DEFAULT 0,
    libelle				        varchar(255),
    amount                      double(24, 8) DEFAULT 0 NOT NULL,
    remise                      real     DEFAULT 0,
    vat_src_code		        varchar(10)  DEFAULT '',
    localtax1				    double(24,8)     DEFAULT 0,
    localtax2				    double(24,8)     DEFAULT 0,
    total_ht				    double(24,8)     DEFAULT 0,
    total_tva				    double(24,8)     DEFAULT 0,
    total_ttc				    double(24,8)     DEFAULT 0,
    fk_user_author              integer,
    fk_user_modif               integer,
    fk_projet                   integer,
    fk_account                  integer,
    fk_cond_reglement		    integer,
    fk_mode_reglement		    integer,
    date_lim_reglement 	        date,
    note_private                text,
    note_public                 text,
    modelpdf                    varchar(255),
    fk_multicurrency            integer,
    multicurrency_code          varchar(3),
    multicurrency_tx            double(24,8) DEFAULT 1,
    multicurrency_total_ht      double(24,8) DEFAULT 0,
    multicurrency_total_tva     double(24,8) DEFAULT 0,
    multicurrency_total_ttc     double(24,8) DEFAULT 0,
    usenewprice                 integer DEFAULT 0,
	usenewcurrencyrate   		integer DEFAULT 0,
    frequency                   integer,
    unit_frequency              varchar(2) DEFAULT 'm',
    date_when                   datetime DEFAULT NULL,
    date_last_gen               datetime DEFAULT NULL,
    nb_gen_done                 integer DEFAULT NULL,
    nb_gen_max                  integer DEFAULT NULL,
    auto_validate               integer DEFAULT 0,
    generate_pdf                integer DEFAULT 1
)ENGINE=innodb;
create table llx_facture_fourn_rec_extrafields
(
    rowid                     integer AUTO_INCREMENT PRIMARY KEY,
    tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object                 integer NOT NULL,
    import_key                varchar(14)
) ENGINE=innodb;
create table llx_facture_rec
(
  rowid              integer AUTO_INCREMENT PRIMARY KEY,
  titre              varchar(200) NOT NULL,
  entity             integer DEFAULT 1 NOT NULL,
  subtype				     smallint DEFAULT NULL,
  fk_soc             integer NOT NULL,
  datec              datetime,
  tms				 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  suspended          integer DEFAULT 0,
  amount             double(24,8)     DEFAULT 0 NOT NULL,
  remise             real     DEFAULT 0,
  remise_percent     real     DEFAULT 0,
  remise_absolue     real     DEFAULT 0,
  vat_src_code		 varchar(10)  DEFAULT '',
  total_tva          double(24,8)     DEFAULT 0,
  localtax1			 double(24,8)     DEFAULT 0,
  localtax2          double(24,8)     DEFAULT 0,
  revenuestamp       double(24,8)     DEFAULT 0,
  total_ht           double(24,8)     DEFAULT 0,
  total_ttc          double(24,8)     DEFAULT 0,
  fk_user_author     integer,
  fk_user_modif      integer,
  fk_projet          integer,
  fk_cond_reglement  integer  DEFAULT 1 NOT NULL,
  fk_mode_reglement  integer DEFAULT 0,
  date_lim_reglement date,
  fk_account         integer,
  fk_societe_rib     integer,
  note_private       text,
  note_public        text,
  modelpdf           varchar(255),
  fk_multicurrency          integer,
  multicurrency_code        varchar(3),
  multicurrency_tx          double(24,8) DEFAULT 1,
  multicurrency_total_ht    double(24,8) DEFAULT 0,
  multicurrency_total_tva   double(24,8) DEFAULT 0,
  multicurrency_total_ttc   double(24,8) DEFAULT 0,
  usenewprice        	integer DEFAULT 0,
  usenewcurrencyrate   	integer DEFAULT 0,
  frequency          	integer,
  unit_frequency     	varchar(2) DEFAULT 'm',
  rule_for_lines_dates	varchar(255) DEFAULT 'prepaid',
  date_when          datetime DEFAULT NULL,
  date_last_gen      datetime DEFAULT NULL,
  nb_gen_done        integer DEFAULT NULL,
  nb_gen_max         integer DEFAULT NULL,
  auto_validate      integer DEFAULT 0,
  fk_email_template  integer DEFAULT NULL,
  generate_pdf       integer DEFAULT 1
)ENGINE=innodb;
create table llx_facture_rec_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_facturedet
(
  rowid							integer    AUTO_INCREMENT PRIMARY KEY,
  fk_facture					integer    NOT NULL,
  fk_parent_line				integer	   NULL,
  fk_product					integer    NULL,
  label							varchar(255) DEFAULT NULL,
  description					text,
  vat_src_code					varchar(10)  DEFAULT '',
  tva_tx						double(7,4),
  localtax1_tx               	double(7,4)  DEFAULT 0,
  localtax1_type			 	varchar(10)	 NULL,
  localtax2_tx               	double(7,4)  DEFAULT 0,
  localtax2_type			 	varchar(10)	 NULL,
  qty							real,
  remise_percent				real       DEFAULT 0,
  remise						real       DEFAULT 0,
  fk_remise_except				integer    NULL,
  subprice						double(24,8),
  subprice_ttc      			double(24,8) DEFAULT 0,
  price							double(24,8),
  total_ht						double(24,8),
  total_tva						double(24,8),
  total_localtax1				double(24,8) DEFAULT 0,
  total_localtax2				double(24,8) DEFAULT 0,
  total_ttc						double(24,8),
  product_type					integer    DEFAULT 0,
  date_start					datetime   DEFAULT NULL,
  date_end						datetime   DEFAULT NULL,
  info_bits						integer    DEFAULT 0,
  buy_price_ht					double(24,8) DEFAULT 0,
  fk_product_fournisseur_price	integer      DEFAULT NULL,
  batch                         varchar(128) NULL,
  fk_warehouse					integer NULL,
  special_code					integer    DEFAULT 0,
  rang							integer    DEFAULT 0,
  fk_contract_line  			integer NULL,
  fk_unit                       integer DEFAULT NULL,
  import_key					varchar(14),
  fk_code_ventilation			integer    DEFAULT 0 NOT NULL,
  situation_percent real DEFAULT 100,
  fk_prev_id        integer,
  fk_user_author	integer,
  fk_user_modif     integer,
  fk_multicurrency				integer,
  multicurrency_code			varchar(3),
  multicurrency_subprice		double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc	double(24,8) DEFAULT 0,
  multicurrency_total_ht		double(24,8) DEFAULT 0,
  multicurrency_total_tva		double(24,8) DEFAULT 0,
  multicurrency_total_ttc		double(24,8) DEFAULT 0,
  ref_ext						varchar(255) DEFAULT NULL,
  extraparams				 	varchar(255)
)ENGINE=innodb;
create table llx_facturedet_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_facturedet_rec
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  fk_facture		integer NOT NULL,
  fk_parent_line	integer NULL,
  fk_product		integer NULL,
  product_type		integer DEFAULT 0,
  label				varchar(255) DEFAULT NULL,
  description		text,
  vat_src_code					varchar(10)  DEFAULT '',
  tva_tx			double(7,4),
  localtax1_tx      double(7,4) DEFAULT 0,
  localtax1_type	varchar(10) NULL,
  localtax2_tx      double(7,4) DEFAULT 0,
  localtax2_type	varchar(10)	 NULL,
  qty				real,
  remise_percent	real DEFAULT 0,
  remise			real DEFAULT 0,
  subprice			double(24,8) DEFAULT 0 NOT NULL,
  subprice_ttc      double(24,8) DEFAULT 0,
  price				double(24,8),
  total_ht			double(24,8),
  total_tva			double(24,8),
  total_localtax1	double(24,8) DEFAULT 0,
  total_localtax2	double(24,8) DEFAULT 0,
  total_ttc			double(24,8),
  date_start_fill	integer DEFAULT 0,
  date_end_fill		integer DEFAULT 0,
  info_bits			integer DEFAULT 0,
  buy_price_ht					double(24,8) DEFAULT 0,
  fk_product_fournisseur_price	integer      DEFAULT NULL,
  special_code		integer UNSIGNED DEFAULT 0,
  rang				integer DEFAULT 0,
  fk_contract_line  integer NULL,
  fk_unit           integer    DEFAULT NULL,
  import_key		varchar(14),
  fk_user_author	integer,
  fk_user_modif     integer,
  fk_multicurrency          integer,
  multicurrency_code        varchar(3),
  multicurrency_subprice    	double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc	double(24,8) DEFAULT 0,
  multicurrency_total_ht    double(24,8) DEFAULT 0,
  multicurrency_total_tva   double(24,8) DEFAULT 0,
  multicurrency_total_ttc   double(24,8) DEFAULT 0,
  extraparams				varchar(255)
)ENGINE=innodb;
create table llx_facturedet_rec_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_fichinter
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  fk_soc			integer NOT NULL,
  fk_projet			integer DEFAULT 0,
  fk_contrat		integer DEFAULT 0,
  ref				varchar(30) NOT NULL,
  ref_ext			varchar(255),
  ref_client		varchar(255),
  entity			integer DEFAULT 1 NOT NULL,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec				datetime,
  date_valid		datetime,
  datei				date,
  fk_user_author	integer,
  fk_user_modif     integer,
  fk_user_valid		integer,
  fk_statut			smallint  DEFAULT 0,
  dateo				date,
  datee				date,
  datet				date,
  duree				real,
  description		text,
  signed_status     smallint DEFAULT NULL,
  online_sign_ip	varchar(48),
  online_sign_name	varchar(64),
  note_private		text,
  note_public		text,
  model_pdf			varchar(255),
  last_main_doc		varchar(255),
  import_key        varchar(14),
  extraparams		varchar(255)
)ENGINE=innodb;
create table llx_fichinter_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_fichinter_rec
(
	rowid				integer AUTO_INCREMENT PRIMARY KEY,
	title				varchar(50) NOT NULL,
	entity				integer DEFAULT 1 NOT NULL,
	fk_soc				integer DEFAULT NULL,
	datec				datetime,
	fk_contrat			integer DEFAULT 0,
	fk_user_author		integer,
	fk_projet			integer,
	duree				real,
	description			text,
	modelpdf			varchar(255),
	note_private		text,
	note_public			text,
	frequency			integer,
	unit_frequency		varchar(2) DEFAULT 'm',
	date_when			datetime DEFAULT NULL,
	date_last_gen		datetime DEFAULT NULL,
	nb_gen_done			integer DEFAULT NULL,
	nb_gen_max			integer DEFAULT NULL,
	auto_validate		integer NULL DEFAULT NULL,
	status				smallint DEFAULT 0
)ENGINE=innodb;
create table llx_fichinterdet
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  fk_fichinter      integer,
  fk_parent_line    integer NULL,
  special_code      integer DEFAULT 0,
  product_type      integer DEFAULT 0,
  date              datetime,
  description       text,
  duree             integer,
  rang              integer DEFAULT 0,
  extraparams		varchar(255)
)ENGINE=innodb;
CREATE TABLE llx_fichinterdet_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_fichinterdet_rec
(
	rowid				integer AUTO_INCREMENT PRIMARY KEY,
	fk_fichinter		integer NOT NULL,
	date				datetime,
	description			text,
	duree				integer,
	rang				integer DEFAULT 0,
	total_ht			DOUBLE(24, 8) NULL DEFAULT NULL,
	subprice			DOUBLE(24, 8) NULL DEFAULT NULL,
    subprice_ttc      	double(24,8) DEFAULT 0,
	fk_parent_line		integer NULL DEFAULT NULL,
	fk_product			integer NULL DEFAULT NULL,
	label				varchar(255) NULL DEFAULT NULL,
	tva_tx				DOUBLE(6, 3) NULL DEFAULT NULL,
	localtax1_tx		DOUBLE(6, 3) NULL DEFAULT 0,
	localtax1_type		VARCHAR(1) NULL DEFAULT NULL,
	localtax2_tx		DOUBLE(6, 3) NULL DEFAULT 0,
	localtax2_type		VARCHAR(1) NULL DEFAULT NULL,
	qty					double NULL DEFAULT NULL,
	remise_percent		double NULL DEFAULT 0,
	fk_remise_except	integer NULL DEFAULT NULL,
	price				DOUBLE(24, 8) NULL DEFAULT NULL,
	total_tva			DOUBLE(24, 8) NULL DEFAULT NULL,
	total_localtax1		DOUBLE(24, 8) NULL DEFAULT 0,
	total_localtax2		DOUBLE(24, 8) NULL DEFAULT 0,
	total_ttc			DOUBLE(24, 8) NULL DEFAULT NULL,
	product_type		INTEGER NULL DEFAULT 0,
	date_start			datetime NULL DEFAULT NULL,
	date_end			datetime NULL DEFAULT NULL,
	info_bits			INTEGER NULL DEFAULT 0,
	buy_price_ht		DOUBLE(24, 8) NULL DEFAULT 0,
	fk_product_fournisseur_price	integer NULL DEFAULT NULL,
	fk_code_ventilation	integer NOT NULL DEFAULT 0,
	special_code		integer UNSIGNED NULL DEFAULT 0,
	fk_unit				integer NULL DEFAULT NULL,
	import_key			varchar(14) NULL DEFAULT NULL,
	extraparams			varchar(255)
)ENGINE=innodb;
CREATE TABLE llx_holiday
(
rowid          integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
ref			   varchar(30) NOT NULL,
ref_ext		   varchar(255),
entity         integer DEFAULT 1 NOT NULL,
fk_user        integer NOT NULL,
fk_user_create integer,
fk_user_modif  integer,
fk_type        integer NOT NULL,
date_create    DATETIME NOT NULL,
description    VARCHAR( 255 ) NOT NULL,
date_debut     DATE NOT NULL,
date_fin       DATE NOT NULL,
halfday        integer DEFAULT 0,
nb_open_day    double(24,8) DEFAULT NULL,
statut         integer NOT NULL DEFAULT 1,
fk_validator   integer NOT NULL,
date_valid     DATETIME DEFAULT NULL,
fk_user_valid  integer DEFAULT NULL,
date_approval  DATETIME DEFAULT NULL,
fk_user_approve integer DEFAULT NULL,
date_refuse    DATETIME DEFAULT NULL,
fk_user_refuse integer DEFAULT NULL,
date_cancel    DATETIME DEFAULT NULL,
fk_user_cancel integer DEFAULT NULL,
detail_refuse  varchar( 250 ) DEFAULT NULL,
note_private   text,
note_public    text,
tms            timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
import_key			varchar(14),
extraparams			varchar(255)
)
ENGINE=innodb;
CREATE TABLE llx_holiday_config
(
	rowid    integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
	entity 	 integer DEFAULT 1 NOT NULL,
	name     VARCHAR(128) NOT NULL,
	value    TEXT NULL
)
ENGINE=innodb;
create table llx_holiday_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_holiday_logs
(
rowid             integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
date_action       DATETIME NOT NULL,
fk_user_action    integer NOT NULL,
fk_user_update    integer NOT NULL,
fk_type           integer NOT NULL,
type_action       VARCHAR( 255 ) NOT NULL,
prev_solde        VARCHAR( 255 ) NOT NULL,
new_solde         VARCHAR( 255 ) NOT NULL
)
ENGINE=innodb;
CREATE TABLE llx_holiday_users
(
	fk_user     integer NOT NULL,
	fk_type     integer NOT NULL,
	nb_holiday  real NOT NULL DEFAULT 0,
	import_key	varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_hrm_evaluation(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity INTEGER DEFAULT 1 NOT NULL,
	ref varchar(128) DEFAULT '(PROV)' NOT NULL,
	label varchar(255),
	description text,
	note_public text,
	note_private text,
	model_pdf varchar(255),
	last_main_doc varchar(255),
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	import_key varchar(14),
	status smallint NOT NULL,
	date_eval date,
	fk_user integer NOT NULL,
	fk_job integer NOT NULL
) ENGINE=innodb;
create table llx_hrm_evaluation_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_hrm_evaluationdet
(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	fk_skill integer NOT NULL,
	fk_evaluation integer NOT NULL,
	rankorder integer NOT NULL,
	required_rank integer NOT NULL,
	comment text,
	import_key varchar(14)
) ENGINE=innodb;
create table llx_hrm_evaluationdet_extrafields
(
    rowid                     integer AUTO_INCREMENT PRIMARY KEY,
    tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object                 integer NOT NULL,
    import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_hrm_job(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    label varchar(255) NOT NULL,
	description text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	deplacement varchar(255),
    note_public text,
    note_private text,
    fk_user_creat integer,
    fk_user_modif integer
) ENGINE=innodb;
create table llx_hrm_job_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_hrm_job_user(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	description text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_contrat integer,
	fk_user integer,
	fk_job integer NOT NULL,
	date_start datetime,
	date_end datetime,
	abort_comment varchar(255),
    note_public text,
    note_private text,
    fk_user_creat integer,
    fk_user_modif integer
) ENGINE=innodb;
CREATE TABLE llx_hrm_skill(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	label varchar(255),
	description text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	required_level integer NOT NULL,
	date_validite integer NOT NULL,
	temps_theorique double(24,8) NOT NULL,
	skill_type integer NOT NULL,
	note_public text,
	note_private text
) ENGINE=innodb;
create table llx_hrm_skill_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_hrm_skilldet(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	fk_skill integer NOT NULL,
	rankorder integer NOT NULL DEFAULT '1',
	description text,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer
) ENGINE=innodb;
CREATE TABLE llx_hrm_skillrank
(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	fk_skill integer NOT NULL,
	rankorder integer NOT NULL,
	fk_object integer NOT NULL,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	objecttype varchar(128) NOT NULL
) ENGINE=innodb;
create table llx_import_model
(
  	rowid         integer AUTO_INCREMENT PRIMARY KEY,
  	entity        integer DEFAULT 0 NOT NULL,
	fk_user		  integer DEFAULT 0 NOT NULL,
  	label         varchar(50) NOT NULL,
  	type		  varchar(64) NOT NULL,
  	field         text NOT NULL
)ENGINE=innodb;
create table llx_intracommreport
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  ref				varchar(30)        NOT NULL,
  entity			integer  DEFAULT 1 NOT NULL,
  type_declaration	varchar(32),
  periods			varchar(32),
  mode				varchar(32),
  content_xml		text,
  type_export		varchar(10),
  datec             datetime,
  tms               timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
CREATE TABLE llx_inventory
(
  rowid integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  entity integer DEFAULT 0,
  ref varchar(48),
  date_creation datetime DEFAULT NULL,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat	integer,
  fk_user_modif integer,
  fk_user_valid integer,
  fk_warehouse integer DEFAULT NULL,
  fk_product integer DEFAULT NULL,
  categories_product varchar(255) DEFAULT NULL,
  status integer DEFAULT 0,
  title varchar(255) NOT NULL,
  date_inventory datetime DEFAULT NULL,
  date_validation datetime DEFAULT NULL,
  import_key               varchar(14)
)
ENGINE=innodb;
create table llx_inventory_extrafields
(
    rowid                     integer AUTO_INCREMENT PRIMARY KEY,
    tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object                 integer NOT NULL,
    import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_inventorydet
(
	rowid integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
	datec datetime DEFAULT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_inventory integer DEFAULT 0,
	fk_warehouse integer DEFAULT 0,
	fk_product integer DEFAULT 0,
	batch varchar(128) DEFAULT NULL,
	qty_stock double DEFAULT NULL,
	qty_view double DEFAULT NULL,
	qty_regulated double DEFAULT NULL,
	pmp_real double DEFAULT NULL,
	pmp_expected double DEFAULT NULL,
	fk_movement integer NULL
)
ENGINE=innodb;
CREATE TABLE llx_knowledgemanagement_knowledgerecord(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity integer DEFAULT 1 NOT NULL,
	ref varchar(128) NOT NULL,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	last_main_doc varchar(255),
	lang varchar(6),
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	fk_user_valid integer,
	import_key varchar(14),
	model_pdf varchar(255),
	question text NOT NULL,
	answer longtext,
	url varchar(255),
	fk_ticket integer,
	fk_c_ticket_category integer,
	status integer NOT NULL
) ENGINE=innodb;
create table llx_knowledgemanagement_knowledgerecord_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_links
(
  rowid             INTEGER AUTO_INCREMENT PRIMARY KEY,
  entity            INTEGER DEFAULT 1 NOT NULL,
  datea             DATETIME NOT NULL,
  url               VARCHAR(255) NOT NULL,
  label             VARCHAR(255) NOT NULL,
  objecttype        VARCHAR(255) NOT NULL,
  objectid          INTEGER NOT NULL,
  share				varchar(128) NULL,
  share_pass		varchar(32) NULL
)ENGINE=innodb;
create table llx_loan
(
  rowid							integer AUTO_INCREMENT PRIMARY KEY,
  entity						integer DEFAULT 1 NOT NULL,
  datec							datetime,
  tms							timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  label							varchar(80) NOT NULL,
  fk_bank						integer,
  capital						double(24,8) DEFAULT 0 NOT NULL,
  insurance_amount				double(24,8) DEFAULT 0,
  datestart						date,
  dateend						date,
  nbterm						real,
  rate							double  NOT NULL,
  note_private					text,
  note_public					text,
  capital_position				double(24,8) DEFAULT 0,
  date_position					date,
  paid							smallint default 0 NOT NULL,
  accountancy_account_capital	varchar(32),
  accountancy_account_insurance	varchar(32),
  accountancy_account_interest	varchar(32),
  fk_projet						integer DEFAULT NULL,
  fk_user_author				integer DEFAULT NULL,
  fk_user_modif					integer DEFAULT NULL,
  active						tinyint DEFAULT 1  NOT NULL
)ENGINE=innodb;
create table llx_loan_schedule
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  fk_loan			integer,
  datec				datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep				datetime,
  amount_capital	double(24,8) DEFAULT 0,
  amount_insurance	double(24,8) DEFAULT 0,
  amount_interest	double(24,8) DEFAULT 0,
  fk_typepayment	integer NOT NULL,
  num_payment		varchar(50),
  note_private      text,
  note_public       text,
  fk_bank			integer NOT NULL,
  fk_payment_loan     integer,
  fk_user_creat		integer,
  fk_user_modif		integer
)ENGINE=innodb;
create table llx_localtax
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  entity          integer DEFAULT 1 NOT NULL,
  localtaxtype    tinyint,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep           date,
  datev           date,
  amount          double,
  label           varchar(255),
  note            text,
  fk_bank         integer,
  fk_user_creat   integer,
  fk_user_modif   integer
)ENGINE=innodb;
create table llx_mailing
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  messtype		    varchar(16) DEFAULT 'email',
  titre				varchar(128),
  entity			integer DEFAULT 1 NOT NULL,
  sujet				varchar(128),
  body				mediumtext,
  bgcolor			varchar(8),
  bgimage			varchar(255),
  evenunsubscribe   smallint       DEFAULT 0,
  cible				varchar(60),
  nbemail			integer,
  email_from		varchar(160),
  name_from         varchar(128),
  email_replyto		varchar(160),
  email_errorsto	varchar(160),
  tag				varchar(128) NULL,
  date_creat		datetime,
  date_valid		datetime,
  date_appro		datetime,
  date_envoi		datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat		integer,
  fk_user_modif		integer,
  fk_user_valid		integer,
  fk_user_appro		integer,
  extraparams		varchar(255),
  joined_file1		varchar(255),
  joined_file2		varchar(255),
  joined_file3		varchar(255),
  joined_file4		varchar(255),
  statut			smallint       DEFAULT 0,
  note_private    text,
  note_public     text,
  fk_project	integer DEFAULT NULL
)ENGINE=innodb;
CREATE TABLE llx_mailing_advtarget
(
  rowid integer NOT NULL auto_increment PRIMARY KEY,
  name varchar(180) NOT NULL,
  entity integer NOT NULL DEFAULT 1,
  fk_element	integer NOT NULL,
  type_element	varchar(180) NOT NULL,
  filtervalue	text,
  fk_user_author integer NOT NULL,
  datec datetime NOT NULL,
  fk_user_mod integer NOT NULL,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP NOT NULL
)ENGINE=innodb;
create table llx_mailing_cibles
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  fk_mailing		integer NOT NULL,
  fk_contact		integer NOT NULL,
  lastname			varchar(160),
  firstname			varchar(160),
  email				varchar(160) NOT NULL,
  other				varchar(255) NULL,
  tag				varchar(64) NULL,
  statut			smallint NOT NULL DEFAULT 0,
  source_url		varchar(255),
  source_id			integer,
  source_type		varchar(32),
  date_envoi		datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  error_text		varchar(255)
)ENGINE=innodb;
create table llx_mailing_unsubscribe
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  entity			integer DEFAULT 1 NOT NULL,
  email				varchar(255),
  unsubscribegroup	varchar(128) DEFAULT '',
  ip				varchar(128),
  date_creat		datetime,
  tms               timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
CREATE TABLE llx_menu
(
	rowid         integer AUTO_INCREMENT NOT NULL PRIMARY KEY,
	menu_handler  varchar(16) NOT NULL,
	entity        integer DEFAULT 1 NOT NULL,
	module        varchar(255),
	type          varchar(4) NOT NULL,
	mainmenu      varchar(100) NOT NULL,
    leftmenu      varchar(100) NULL,
	fk_menu       integer NOT NULL,
    fk_mainmenu   varchar(100),
    fk_leftmenu   varchar(100),
	position      integer NOT NULL,
	url           TEXT NOT NULL,
	showtopmenuinframe integer DEFAULT 0,
	target        varchar(100) NULL,
	titre         varchar(255) NOT NULL,
	prefix        varchar(255) NULL,
	langs         varchar(100),
	level         smallint,
	perms         text,
	enabled       text NULL,
	usertype      integer NOT NULL DEFAULT 0,
	tms           timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=innodb;
CREATE TABLE llx_mrp_mo(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity integer DEFAULT 1 NOT NULL,
	ref varchar(128) DEFAULT '(PROV)' NOT NULL,
	mrptype integer DEFAULT 0,
	label varchar(255),
	qty real NOT NULL,
	fk_warehouse integer,
	fk_soc integer,
	note_public text,
	note_private text,
	date_creation datetime NOT NULL,
	date_valid datetime NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	fk_user_valid integer,
	import_key varchar(14),
	model_pdf varchar(255),
	status integer NOT NULL,
	fk_product integer NOT NULL,
	date_start_planned datetime,
	date_end_planned datetime,
	fk_bom integer,
	fk_project integer,
	last_main_doc varchar(255),
	fk_parent_line integer,
	extraparams varchar(255) DEFAULT NULL
) ENGINE=innodb;
create table llx_mrp_mo_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_mrp_production(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	fk_mo integer NOT NULL,
	origin_id integer,
	origin_type varchar(10),
	position integer NOT NULL DEFAULT 0,
	fk_product integer NOT NULL,
	fk_warehouse integer,
	qty real NOT NULL DEFAULT 1,
	qty_frozen smallint DEFAULT 0,
    disable_stock_change smallint DEFAULT 0,
	batch varchar(128),
	role varchar(10),
	fk_mrp_production integer,
	fk_stock_movement integer,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	import_key varchar(14),
	fk_default_workstation integer DEFAULT NULL,
	fk_unit integer DEFAULT NULL
) ENGINE=innodb;
create table llx_mrp_production_extrafields
(
    rowid                     integer AUTO_INCREMENT PRIMARY KEY,
    tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object                 integer NOT NULL,
    import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_multicurrency
(
	rowid 		integer AUTO_INCREMENT PRIMARY KEY,
	date_create datetime DEFAULT NULL,
	code 		varchar(255) DEFAULT NULL,
	name 		varchar(255) DEFAULT NULL,
	entity 		integer DEFAULT 1,
	fk_user 	integer DEFAULT NULL,
	tms         timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=innodb;
CREATE TABLE llx_multicurrency_rate
(
	rowid 				integer AUTO_INCREMENT PRIMARY KEY,
	date_sync 			datetime DEFAULT NULL,
	rate 				double NOT NULL DEFAULT 0,
	rate_direct 		double DEFAULT 0,
	fk_multicurrency 	integer NOT NULL,
	entity 				integer DEFAULT 1,
	tms           		timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
) ENGINE=innodb;
create table llx_notify
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  daten           datetime,
  fk_action       integer NOT NULL,
  fk_soc          integer NULL,
  fk_contact      integer NULL,
  fk_user         integer NULL,
  type            varchar(16) DEFAULT 'email',
  type_target     varchar(16) NULL,
  objet_type      varchar(24) NOT NULL,
  objet_id        integer NOT NULL,
  email           varchar(255)
)ENGINE=innodb;
create table llx_notify_def
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  entity 		  integer DEFAULT 1,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec           date,
  fk_action       integer NOT NULL,
  fk_soc          integer,
  fk_contact      integer,
  fk_user		  integer,
  email           varchar(255),
  threshold       double(24,8),
  context         varchar(128),
  type            varchar(16) DEFAULT 'email'
)ENGINE=innodb;
create table llx_notify_def_object
(
  id				integer AUTO_INCREMENT PRIMARY KEY,
  entity			integer DEFAULT 1 NOT NULL,
  objet_type		varchar(16),
  objet_id			integer NOT NULL,
  type_notif		varchar(16) DEFAULT 'browser',
  date_notif		datetime,
  user_id			integer,
  moreparam			varchar(255)
)ENGINE=innodb;
CREATE TABLE llx_oauth_state (
    rowid integer AUTO_INCREMENT PRIMARY KEY,
    service varchar(36),
    state varchar(128),
    fk_user integer,
    fk_adherent integer,
    entity integer DEFAULT 1
)ENGINE=innodb;
CREATE TABLE llx_oauth_token (
    rowid 			integer AUTO_INCREMENT PRIMARY KEY,
    service 		varchar(36),
    token 			text,
    tokenstring 	text,
    tokenstring_refresh	text NULL,
    expire_at 		datetime NULL,
    state           text,
    fk_soc 			integer,
    fk_user 		integer,
    fk_adherent 	integer,
    restricted_ips 	varchar(200),
    datec       	datetime DEFAULT NULL,
    tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    entity 			integer DEFAULT 1,
    lastaccess    	datetime NULL,
    apicount_previous_month BIGINT UNSIGNED DEFAULT 0,
    apicount_month BIGINT UNSIGNED DEFAULT 0,
    apicount_total BIGINT UNSIGNED DEFAULT 0
)ENGINE=innodb;
create table llx_object_lang
(
  rowid          integer AUTO_INCREMENT PRIMARY KEY,
  fk_object      integer      DEFAULT 0 NOT NULL,
  type_object    varchar(32)  NOT NULL,
  property       varchar(32)  NOT NULL,
  lang           varchar(5)   DEFAULT '' NOT NULL,
  value          text,
  import_key varchar(14) DEFAULT NULL
)ENGINE=innodb;
create table llx_onlinesignature
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  entity                    integer DEFAULT 1 NOT NULL,
  object_type               varchar(32) NOT NULL,
  object_id					integer NOT NULL,
  datec                     datetime NOT NULL,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  name						varchar(255) NOT NULL,
  ip						varchar(128),
  pathoffile				varchar(255)
)ENGINE=innodb;
CREATE TABLE llx_opensurvey_comments (
    id_comment INTEGER unsigned NOT NULL AUTO_INCREMENT PRIMARY KEY,
    id_sondage CHAR(16) NOT NULL,
    comment text NOT NULL,
    tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    usercomment text,
    date_creation datetime NOT NULL,
    ip varchar(250)
) ENGINE=innodb;
CREATE TABLE llx_opensurvey_formquestions (
	rowid INTEGER AUTO_INCREMENT NOT NULL PRIMARY KEY,
	id_sondage VARCHAR(16),
	question TEXT,
    available_answers TEXT
) ENGINE=innodb;
CREATE TABLE llx_opensurvey_sondage (
	id_sondage VARCHAR(16) PRIMARY KEY,
	entity integer DEFAULT 1 NOT NULL,
    commentaires text,
	mail_admin VARCHAR(128),
	nom_admin VARCHAR(64),
	fk_user_creat integer NOT NULL,
	titre TEXT NOT NULL,
	date_fin DATETIME NULL,
    status integer DEFAULT 1,
	format VARCHAR(2) NOT NULL,
	mailsonde tinyint NOT NULL DEFAULT 0,
	allow_comments tinyint NOT NULL DEFAULT 1,
	allow_spy tinyint NOT NULL DEFAULT 1,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	sujet TEXT
) ENGINE=innodb;
CREATE TABLE llx_opensurvey_user_formanswers (
    fk_user_survey INTEGER NOT NULL,
    fk_question INTEGER NOT NULL,
    reponses TEXT
) ENGINE=innodb;
CREATE TABLE llx_opensurvey_user_studs (
    id_users INTEGER NOT NULL AUTO_INCREMENT PRIMARY KEY,
    nom VARCHAR(64) NOT NULL,
    id_sondage VARCHAR(16) NOT NULL,
    reponses VARCHAR(200) NOT NULL,
    tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    date_creation datetime NOT NULL,
    ip varchar(250)
) ENGINE=innodb;
create table llx_overwrite_trans
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  entity          integer DEFAULT 1 NOT NULL,
  lang            varchar(5),
  transkey	      varchar(128),
  transvalue      text
)ENGINE=innodb;
create table llx_paiement
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  ref              varchar(30) NULL,
  ref_ext          varchar(255) NULL,
  entity           integer   DEFAULT 1 NOT NULL,
  datec            datetime,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep            datetime,
  amount           double(24,8) DEFAULT 0,
  multicurrency_amount double(24,8) DEFAULT 0,
  fk_paiement      integer NOT NULL,
  num_paiement     varchar(50),
  note             text,
  ext_payment_id   varchar(255),
  ext_payment_site varchar(128),
  fk_bank          integer NOT NULL DEFAULT 0,
  fk_user_creat    integer,
  fk_user_modif    integer,
  statut           smallint DEFAULT 0 NOT NULL,
  fk_export_compta integer DEFAULT 0 NOT NULL,
  pos_change       double(24,8) DEFAULT 0
)ENGINE=innodb;
create table llx_paiement_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_paiement_facture
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  fk_paiement     integer,
  fk_facture      integer,
  amount          double(24,8)     DEFAULT 0,
  multicurrency_code		varchar(3),
  multicurrency_tx		double(24,8) DEFAULT 1,
  multicurrency_amount	double(24,8) DEFAULT 0
)ENGINE=innodb;
create table llx_paiementcharge
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  fk_charge       integer,
  datec           datetime,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep           datetime,
  amount          double(24,8) DEFAULT 0,
  fk_typepaiement integer NOT NULL,
  num_paiement    varchar(50),
  note            text,
  fk_bank         integer NOT NULL,
  fk_user_creat   integer,
  fk_user_modif   integer
)ENGINE=innodb;
create table llx_paiementfourn
(
  rowid					integer AUTO_INCREMENT PRIMARY KEY,
  ref					varchar(30),
  entity				integer DEFAULT 1,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec					datetime,
  datep					datetime,
  amount				double(24,8) DEFAULT 0,
  multicurrency_amount	double(24,8) DEFAULT 0,
  fk_user_author		integer,
  fk_user_modif 		integer,
  fk_paiement			integer NOT NULL,
  num_paiement			varchar(50),
  note					text,
  fk_bank				integer NOT NULL,
  statut				smallint NOT NULL DEFAULT 0,
  model_pdf				varchar(255)
)ENGINE=innodb;
create table llx_paiementfourn_facturefourn
(
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  fk_paiementfourn INTEGER DEFAULT NULL,
  fk_facturefourn  INTEGER DEFAULT NULL,
  amount double(24,8) DEFAULT 0,
  multicurrency_code	varchar(3),
  multicurrency_tx		double(24,8) DEFAULT 1,
  multicurrency_amount	double(24,8) DEFAULT 0
)ENGINE=innodb;
CREATE TABLE llx_partnership
(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity integer DEFAULT 1 NOT NULL,
	ref varchar(128) DEFAULT '(PROV)' NOT NULL,
	status smallint DEFAULT 0 NOT NULL,
	fk_type integer DEFAULT 0 NOT NULL,
	fk_soc integer,
	fk_member integer,
	email_partnership varchar(64),
	date_partnership_start date NOT NULL,
	date_partnership_end date NULL,
	reason_decline_or_cancel text NULL,
	date_creation datetime NOT NULL,
	fk_user_creat integer NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_modif integer,
	note_private text,
	note_public text,
	last_main_doc varchar(255),
	url_to_check varchar(255),
	count_last_url_check_error integer DEFAULT '0',
	last_check_backlink datetime NULL,
	ip varchar(250),
	import_key varchar(14),
	model_pdf varchar(255)
) ENGINE=innodb;
create table llx_partnership_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_payment_donation
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  fk_donation     integer,
  datec           datetime,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep           datetime,
  amount          double(24,8) DEFAULT 0,
  fk_typepayment  integer NOT NULL,
  num_payment     varchar(50),
  note            text,
  ext_payment_id   varchar(255),
  ext_payment_site varchar(128),
  fk_bank         integer NOT NULL,
  fk_user_creat   integer,
  fk_user_modif   integer
)ENGINE=innodb;
create table llx_payment_expensereport
(
  rowid                   integer AUTO_INCREMENT PRIMARY KEY,
  fk_expensereport        integer,
  datec                   datetime,
  tms                     timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep                   datetime,
  amount                  double(24,8) DEFAULT 0,
  fk_typepayment          integer NOT NULL,
  num_payment             varchar(50),
  note                    text,
  fk_bank                 integer NOT NULL,
  fk_user_creat           integer,
  fk_user_modif           integer
)ENGINE=innodb;
create table llx_payment_loan
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  fk_loan			integer,
  datec				datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep				datetime,
  amount_capital	double(24,8) DEFAULT 0,
  amount_insurance	double(24,8) DEFAULT 0,
  amount_interest	double(24,8) DEFAULT 0,
  fk_typepayment	integer NOT NULL,
  num_payment		varchar(50),
  note_private		text,
  note_public		text,
  fk_bank			integer NOT NULL,
  fk_user_creat		integer,
  fk_user_modif		integer
)ENGINE=innodb;
create table llx_payment_salary
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  ref             varchar(30) NULL,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec           datetime,
  fk_user         integer DEFAULT NULL,
  datep           datetime,
  datev           date,
  salary          double(24,8),
  amount          double(24,8) NOT NULL DEFAULT 0,
  fk_projet       integer DEFAULT NULL,
  fk_typepayment  integer NOT NULL,
  num_payment     varchar(50),
  label           varchar(255),
  datesp          date,
  dateep          date,
  entity          integer DEFAULT 1 NOT NULL,
  note            text,
  fk_bank         integer,
  fk_user_author  integer,
  fk_user_modif   integer,
  fk_salary       integer
)ENGINE=innodb;
create table llx_payment_various
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  ref                   varchar(30) NULL,
  num_payment           varchar(50),
  label                 varchar(255),
  tms                   timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec                 datetime,
  datep                 date,
  datev                 date,
  sens                  smallint DEFAULT 0 NOT NULL,-- Sens of the operation: 0 for debit operation, 1 for credit operation
  amount                double(24,8) DEFAULT 0 NOT NULL,
  fk_typepayment        integer NOT NULL,
  accountancy_code      varchar(32),
  subledger_account     varchar(32),
  fk_projet             integer DEFAULT NULL,
  entity                integer DEFAULT 1 NOT NULL,
  note                  text,
  fk_bank               integer,
  fk_user_author        integer,
  fk_user_modif         integer
)ENGINE=innodb;
CREATE TABLE llx_payment_various_extrafields
(
	rowid                     integer AUTO_INCREMENT PRIMARY KEY,
	tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_object                 integer NOT NULL,
	import_key                varchar(14)
) ENGINE=innodb;
create table llx_payment_vat
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  fk_tva          integer,
  datec           datetime,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datep           datetime,
  amount          double(24,8) DEFAULT 0,
  fk_typepaiement integer NOT NULL,
  num_paiement    varchar(50),
  note            text,
  fk_bank         integer NOT NULL,
  fk_user_creat   integer,
  fk_user_modif   integer
)ENGINE=innodb;
create table llx_paymentexpensereport_expensereport
(
  rowid            		integer AUTO_INCREMENT PRIMARY KEY,
  fk_payment       		integer,
  fk_expensereport 		integer,
  amount           		double(24,8)     DEFAULT 0,
  multicurrency_code	varchar(3),
  multicurrency_tx		double(24,8) DEFAULT 1,
  multicurrency_amount	double(24,8) DEFAULT 0
)ENGINE=innodb;
CREATE TABLE llx_pos_cash_fence(
	rowid INTEGER AUTO_INCREMENT PRIMARY KEY,
	entity INTEGER DEFAULT 1 NOT NULL,
	ref VARCHAR(64),
	label VARCHAR(255),
	opening double(24,8) default 0,
	cash double(24,8) default 0,
	card double(24,8) default 0,
	cheque double(24,8) default 0,
	cash_declared double(24,8) default NULL,
	card_declared double(24,8) default NULL,
	cheque_declared double(24,8) default NULL,
	cash_lifetime double(24,8) default NULL,
	card_lifetime double(24,8) default NULL,
	cheque_lifetime double(24,8) default NULL,
	lifetime_start datetime default NULL,
	status INTEGER,
	date_creation DATETIME NOT NULL,
	date_valid DATETIME,
	day_close INTEGER,
	month_close INTEGER,
	year_close INTEGER,
	hour_close INTEGER DEFAULT null,
	min_close INTEGER DEFAULT null,
	sec_close INTEGER DEFAULT null,
	posmodule VARCHAR(30),
	posnumber VARCHAR(30),
	fk_user_creat integer,
	fk_user_valid integer,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	import_key VARCHAR(14)
) ENGINE=innodb;
create table llx_pos_cash_fence_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_prelevement
(
  rowid                  integer AUTO_INCREMENT PRIMARY KEY,
  fk_facture             integer NULL,
  fk_facture_fourn       integer NULL,
  fk_salary              integer NULL,
  fk_prelevement_lignes  integer NOT NULL
)ENGINE=innodb;
create table llx_prelevement_bons
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  type            varchar(16) DEFAULT 'debit-order',
  ref             varchar(12),
  entity          integer DEFAULT 1 NOT NULL,
  datec           datetime,
  amount          double(24,8) DEFAULT 0,
  statut          smallint DEFAULT 0,
  credite         smallint DEFAULT 0,
  note            text,
  date_trans      datetime,
  method_trans    smallint,
  fk_user_trans   integer,
  date_credit     datetime,
  fk_user_credit  integer,
  fk_bank_account integer DEFAULT NULL,
  fk_user_modif   integer,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
create table llx_prelevement_demande
(
  rowid               integer AUTO_INCREMENT PRIMARY KEY,
  entity              integer DEFAULT 1 NOT NULL,
  fk_facture          integer NULL,
  fk_facture_fourn    integer NULL,
  fk_salary           integer NULL,
  sourcetype          varchar(32),
  amount              double(24,8) NOT NULL,
  date_demande        datetime NOT NULL,
  traite              smallint DEFAULT 0,
  date_traite         datetime,
  fk_prelevement_bons integer,
  fk_user_demande     integer NOT NULL,
  fk_societe_rib      integer DEFAULT NULL,
  code_banque         varchar(128),
  code_guichet        varchar(6),
  number              varchar(255),
  cle_rib             varchar(5),
  type                varchar(12) DEFAULT '',
  ext_payment_id      varchar(255),
  ext_payment_site    varchar(128)
)ENGINE=innodb;
create table llx_prelevement_lignes
(
  rowid               integer AUTO_INCREMENT PRIMARY KEY,
  fk_prelevement_bons integer,
  fk_soc              integer NOT NULL,
  fk_user             integer NULL,
  statut              smallint DEFAULT 0,
  client_nom          varchar(255),
  amount              double(24,8) DEFAULT 0,
  fk_prelevement_demande integer DEFAULT 0,
  note                text,
  tms             	  timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
create table llx_prelevement_rejet
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  fk_prelevement_lignes integer,
  date_rejet            datetime,
  motif                 integer,
  date_creation         datetime,
  fk_user_creation      integer,
  note                  text,
  afacturer             tinyint DEFAULT 0,
  fk_facture            integer
)ENGINE=innodb;
CREATE TABLE llx_printing
(
 rowid integer AUTO_INCREMENT PRIMARY KEY,
 tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
 datec datetime,
 printer_name text NOT NULL,
 printer_location text NOT NULL,
 printer_id varchar(255) NOT NULL,
 copy integer NOT NULL DEFAULT '1',
 module varchar(16) NOT NULL,
 driver varchar(16) NOT NULL,
 userid integer
)ENGINE=innodb;
CREATE TABLE llx_printer_receipt (
	rowid integer AUTO_INCREMENT PRIMARY KEY,
	name varchar(128),
	fk_type integer,
	fk_profile integer,
	parameter varchar(128),
	entity integer
) ENGINE=innodb;
CREATE TABLE llx_printer_receipt_template (
	rowid integer AUTO_INCREMENT PRIMARY KEY,
	name varchar(128),
	template text,
	entity integer
) ENGINE=innodb;
create table llx_product
(
  rowid                         integer AUTO_INCREMENT PRIMARY KEY,
  ref                           varchar(128)  NOT NULL,
  entity                        integer   DEFAULT 1 NOT NULL,
  ref_ext                       varchar(128),
  datec                         datetime,
  tms                           timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_parent                     integer	  DEFAULT 0,
  label                         varchar(255) NOT NULL,
  description                   text,
  note_public                   mediumtext,
  note                          mediumtext,
  customcode                    varchar(32),
  fk_country                    integer DEFAULT NULL,
  fk_state                      integer DEFAULT NULL,
  price                         double(24,8) DEFAULT 0,
  price_ttc                     double(24,8) DEFAULT 0,
  price_min                     double(24,8) DEFAULT 0,
  price_min_ttc                 double(24,8) DEFAULT 0,
  price_base_type               varchar(3)   DEFAULT 'HT',
  price_label                   varchar(255),
  cost_price                    double(24,8) DEFAULT NULL,
  default_vat_code              varchar(10),
  tva_tx                        double(7,4),
  recuperableonly               integer NOT NULL DEFAULT '0',
  localtax1_tx                  double(7,4)  DEFAULT 0,
  localtax1_type                varchar(10)  NOT NULL DEFAULT '0',
  localtax2_tx                  double(7,4)  DEFAULT 0,
  localtax2_type                varchar(10)  NOT NULL DEFAULT '0',
  fk_user_author                integer DEFAULT NULL,
  fk_user_modif                 integer,
  tosell                        tinyint      DEFAULT 1,
  tobuy                         tinyint      DEFAULT 1,
  tobatch                       tinyint      DEFAULT 0 NOT NULL,
  sell_or_eat_by_mandatory      tinyint      DEFAULT 0 NOT NULL,
  batch_mask			        varchar(32)  DEFAULT NULL,
  fk_product_type               integer      DEFAULT 0,
  duration                      varchar(6),
  seuil_stock_alerte            float      DEFAULT NULL,
  url                           varchar(255),
  barcode                       varchar(180) DEFAULT NULL,
  fk_barcode_type               integer      DEFAULT NULL,
  accountancy_code_sell         varchar(32),
  accountancy_code_sell_intra   varchar(32),
  accountancy_code_sell_export  varchar(32),
  accountancy_code_buy          varchar(32),
  accountancy_code_buy_intra    varchar(32),
  accountancy_code_buy_export   varchar(32),
  partnumber                    varchar(32),
  net_measure                   float        DEFAULT NULL,
  net_measure_units             tinyint      DEFAULT NULL,
  weight                        float        DEFAULT NULL,
  weight_units                  tinyint      DEFAULT NULL,
  length                        float        DEFAULT NULL,
  length_units                  tinyint      DEFAULT NULL,
  width                         float        DEFAULT NULL,
  width_units                   tinyint      DEFAULT NULL,
  height                        float        DEFAULT NULL,
  height_units                  tinyint      DEFAULT NULL,
  surface                       float        DEFAULT NULL,
  surface_units                 tinyint      DEFAULT NULL,
  volume                        float        DEFAULT NULL,
  volume_units                  tinyint      DEFAULT NULL,
  stockable_product             integer      DEFAULT 1 NOT NULL,
  stock                         real,
  pmp                           double(24,8) DEFAULT 0 NOT NULL,
  fifo                          double(24,8),
  lifo                          double(24,8),
  fk_default_warehouse          integer      DEFAULT NULL,
  fk_default_bom                integer      DEFAULT NULL,
  fk_default_workstation        integer      DEFAULT NULL,
  canvas                        varchar(32)  DEFAULT NULL,
  finished                      tinyint      DEFAULT NULL,
  lifetime                      integer      DEFAULT NULL,
  qc_frequency 					integer 	 DEFAULT NULL,
  hidden                        tinyint      DEFAULT 0,
  import_key                    varchar(14),
  model_pdf                     varchar(255),
  fk_price_expression           integer,
  desiredstock                  float        DEFAULT 0,
  fk_unit                       integer      DEFAULT NULL,
  price_autogen                 tinyint      DEFAULT 0,
  fk_project                    integer      DEFAULT NULL,
  mandatory_period              tinyint      DEFAULT 0,
  last_main_doc                 varchar(255),
  packaging			                float(24,8) DEFAULT NULL
)ENGINE=innodb;
create table llx_product_association
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  fk_product_pere       integer NOT NULL DEFAULT 0,
  fk_product_fils       integer NOT NULL DEFAULT 0,
  qty                   double NULL,
  incdec                integer DEFAULT 1,
  rang                  integer DEFAULT 0,
  import_key            varchar(14) DEFAULT NULL
)ENGINE=innodb;
CREATE TABLE llx_product_attribute
(
  rowid		INTEGER			PRIMARY KEY NOT NULL AUTO_INCREMENT,
  ref		VARCHAR(255)	NOT NULL,
  ref_ext	VARCHAR(255)	NULL,
  label		VARCHAR(255)	NOT NULL,
  position	INTEGER			DEFAULT 0 NOT NULL,
  entity	INTEGER			DEFAULT 1 NOT NULL
)ENGINE=innodb;
CREATE TABLE llx_product_attribute_combination
(
  rowid INTEGER PRIMARY KEY NOT NULL AUTO_INCREMENT,
  fk_product_parent INTEGER NOT NULL,
  fk_product_child INTEGER NOT NULL,
  variation_price DOUBLE(24,8) NOT NULL,
  variation_price_percentage INTEGER NULL,
  variation_weight REAL NOT NULL,
  variation_ref_ext VARCHAR(255) NULL,
  entity INTEGER DEFAULT 1 NOT NULL
)ENGINE=innodb;
CREATE TABLE llx_product_attribute_combination2val
(
  rowid INT PRIMARY KEY NOT NULL AUTO_INCREMENT,
  fk_prod_combination INT NOT NULL,
  fk_prod_attr INT NOT NULL,
  fk_prod_attr_val INT NOT NULL
)ENGINE=innodb;
CREATE TABLE llx_product_attribute_combination_price_level
(
  rowid INTEGER PRIMARY KEY NOT NULL AUTO_INCREMENT,
  fk_product_attribute_combination INTEGER DEFAULT 1 NOT NULL,
  fk_price_level INTEGER DEFAULT 1 NOT NULL,
  variation_price DOUBLE(24,8) NOT NULL,
  variation_price_percentage INTEGER NULL
)ENGINE=innodb;
CREATE TABLE llx_product_attribute_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_product_attribute_value
(
  rowid					INTEGER			PRIMARY KEY NOT NULL AUTO_INCREMENT,
  fk_product_attribute	INTEGER			NOT NULL,
  ref					VARCHAR(180)	NOT NULL,
  value					VARCHAR(255)	NOT NULL,
  entity				INTEGER			DEFAULT 1 NOT NULL,
  position				INTEGER			NOT NULL DEFAULT 0
)ENGINE=innodb;
CREATE TABLE llx_product_attribute_value_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_product_batch (
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_product_stock integer NOT NULL,
  eatby datetime DEFAULT NULL,
  sellby datetime DEFAULT NULL,
  batch varchar(128) NOT NULL,
  qty double NOT NULL DEFAULT 0,
  import_key varchar(14) DEFAULT NULL
) ENGINE=innodb;
create table llx_product_customer_price
(
  rowid					integer AUTO_INCREMENT PRIMARY KEY,
  entity				integer DEFAULT 1 NOT NULL,
  datec					datetime,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_product			integer NOT NULL,
  fk_soc				integer NOT NULL,
  ref_customer			varchar(128),
  date_begin			date,
  date_end				date,
  price					double(24,8) DEFAULT 0,
  price_ttc				double(24,8) DEFAULT 0,
  price_min				double(24,8) DEFAULT 0,
  price_min_ttc			double(24,8) DEFAULT 0,
  price_base_type		varchar(3)   DEFAULT 'HT',
  default_vat_code		varchar(10),
  tva_tx				double(7,4),
  recuperableonly       integer NOT NULL DEFAULT '0',
  localtax1_tx          varchar(20)  DEFAULT '0',
  localtax1_type        varchar(10)  NOT NULL DEFAULT '0',
  localtax2_tx			varchar(20)  DEFAULT '0',
  localtax2_type        varchar(10)  NOT NULL DEFAULT '0',
  discount_percent		real DEFAULT 0,
  fk_user				integer,
  price_label           varchar(255),
  import_key			varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_product_customer_price_extrafields (
	rowid               integer AUTO_INCREMENT PRIMARY KEY,
	tms                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_object           integer NOT NULL,
	import_key          varchar(14)
) ENGINE=innodb;
create table llx_product_customer_price_log
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  entity				integer DEFAULT 1 NOT NULL,
  datec                 datetime,
  fk_product			integer NOT NULL,
  fk_soc				integer DEFAULT 0 NOT NULL,
  ref_customer			varchar(30),
  date_begin			date,
  date_end				date,
  price					double(24,8) DEFAULT 0,
  price_ttc				double(24,8) DEFAULT 0,
  price_min				double(24,8) DEFAULT 0,
  price_min_ttc			double(24,8) DEFAULT 0,
  price_base_type		varchar(3)   DEFAULT 'HT',
  default_vat_code		varchar(10),
  tva_tx				double(7,4),
  recuperableonly       integer NOT NULL DEFAULT '0',
  localtax1_tx			varchar(20) DEFAULT '0',
  localtax1_type        varchar(10)  NOT NULL DEFAULT '0',
  localtax2_tx			varchar(20) DEFAULT '0',
  localtax2_type        varchar(10)  NOT NULL DEFAULT '0',
  discount_percent		real DEFAULT 0,
  fk_user				integer,
  price_label           varchar(255),
  import_key 			varchar(14)
)ENGINE=innodb;
create table llx_product_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_product_fournisseur_price
(
  rowid					integer AUTO_INCREMENT PRIMARY KEY,
  entity				integer DEFAULT 1 NOT NULL,
  datec					datetime,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_product			integer,
  fk_soc				integer,
  ref_fourn				varchar(128),
  desc_fourn            text,
  fk_availability		integer,
  price					double(24,8) DEFAULT 0,
  quantity				double,
  remise_percent		double NOT NULL DEFAULT 0,
  remise				double NOT NULL DEFAULT 0,
  unitprice				double(24,8) DEFAULT 0,
  charges				double(24,8) DEFAULT 0,
  default_vat_code	    varchar(10),
  barcode                       varchar(180) DEFAULT NULL,
  fk_barcode_type               integer      DEFAULT NULL,
  tva_tx				double(7,4) NOT NULL,
  localtax1_tx		    double(7,4) DEFAULT 0,
  localtax1_type        varchar(10)  NOT NULL DEFAULT '0',
  localtax2_tx		    double(7,4) DEFAULT 0,
  localtax2_type        varchar(10)  NOT NULL DEFAULT '0',
  info_bits				integer NOT NULL DEFAULT 0,
  fk_user				integer,
  fk_supplier_price_expression	integer,
  delivery_time_days    integer,
  supplier_reputation varchar(10),
  packaging			    real DEFAULT NULL,
  fk_multicurrency		integer,
  multicurrency_code	varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_unitprice   double(24,8) DEFAULT NULL,
  multicurrency_price		double(24,8) DEFAULT NULL,
  import_key			varchar(14),
  status                integer DEFAULT 1
)ENGINE=innodb;
Create Table llx_product_fournisseur_price_extrafields (
	rowid               integer AUTO_INCREMENT PRIMARY KEY,
	tms                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_object           integer NOT NULL,
	import_key          varchar(14)
) ENGINE=innodb;
create table llx_product_fournisseur_price_log
(
  rowid                       integer AUTO_INCREMENT PRIMARY KEY,
  datec                       datetime,
  fk_product_fournisseur      integer      NOT NULL,
  price                       double(24,8) DEFAULT 0,
  quantity                    double,
  fk_user                     integer,
  fk_multicurrency			integer,
  multicurrency_code		varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_unitprice	double(24,8) DEFAULT NULL,
  multicurrency_price		double(24,8) DEFAULT NULL
)ENGINE=innodb;
create table llx_product_lang
(
  rowid          integer AUTO_INCREMENT PRIMARY KEY,
  fk_product     integer      DEFAULT 0 NOT NULL,
  lang           varchar(5)   DEFAULT 0 NOT NULL,
  label          varchar(255) NOT NULL,
  description    text,
  note           mediumtext,
  import_key varchar(14) DEFAULT NULL
)ENGINE=innodb;
create table llx_product_lang_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_product_lot (
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  entity          integer DEFAULT 1,
  fk_product      integer NOT NULL,
  batch           varchar(128) DEFAULT NULL,
  note_public     text DEFAULT NULL,
  note_private    text DEFAULT NULL,
  eatby           date DEFAULT NULL,
  sellby          date DEFAULT NULL,
  eol_date      datetime NULL,
  manufacturing_date datetime NULL,
  scrapping_date datetime NULL,
  qc_frequency   integer DEFAULT NULL,
  lifetime       integer DEFAULT NULL,
  barcode       varchar(180) DEFAULT NULL,
  fk_barcode_type   integer DEFAULT NULL,
  model_pdf			varchar(255),
  last_main_doc		varchar(255),
  datec         datetime,
  tms           timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat integer,
  fk_user_modif integer,
  import_key    integer
) ENGINE=innodb;
create table llx_product_lot_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_product_perentity
(
  rowid         				integer AUTO_INCREMENT PRIMARY KEY,
  fk_product	   				integer,
  entity             			integer DEFAULT 1 NOT NULL,
  accountancy_code_sell         varchar(32),
  accountancy_code_sell_intra   varchar(32),
  accountancy_code_sell_export  varchar(32),
  accountancy_code_buy          varchar(32),
  accountancy_code_buy_intra    varchar(32),
  accountancy_code_buy_export   varchar(32),
  pmp double(24,8)
)ENGINE=innodb;
create table llx_product_price
(
  rowid				    integer AUTO_INCREMENT PRIMARY KEY,
  entity			    integer   DEFAULT 1 NOT NULL,
  tms				    timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_product    		integer NOT NULL,
  date_price	    	datetime NOT NULL,
  price_level		    smallint NULL DEFAULT 1,
  price				    double(24,8) DEFAULT NULL,
  price_ttc			    double(24,8) DEFAULT NULL,
  price_min			    double(24,8) default NULL,
  price_min_ttc		    double(24,8) default NULL,
  price_base_type	    varchar(3) DEFAULT 'HT',
  default_vat_code	    varchar(10),
  tva_tx			    double(7,4) DEFAULT 0 NOT NULL,
  recuperableonly	    integer NOT NULL DEFAULT '0',
  localtax1_tx		    double(7,4) DEFAULT 0,
  localtax1_type        varchar(10)  NOT NULL DEFAULT '0',
  localtax2_tx		    double(7,4) DEFAULT 0,
  localtax2_type        varchar(10)  NOT NULL DEFAULT '0',
  fk_user_author	    integer,
  price_label           varchar(255),
  tosell			    tinyint DEFAULT 1,
  price_by_qty		    integer NOT NULL DEFAULT 0,
  fk_price_expression   integer,
  import_key 		    varchar(14),
  fk_multicurrency		integer,
  multicurrency_code	varchar(3),
  multicurrency_tx		double(24,8) DEFAULT 1,
  multicurrency_price	double(24,8) DEFAULT NULL,
  multicurrency_price_ttc	double(24,8) DEFAULT NULL
)ENGINE=innodb;
create table llx_product_price_by_qty
(
  rowid				integer NOT NULL AUTO_INCREMENT PRIMARY KEY,
  fk_product_price	integer NOT NULL,
  price				double(24,8) DEFAULT 0,
  price_base_type	varchar(3) DEFAULT 'HT',
  quantity			double DEFAULT NULL,
  remise_percent	double NOT NULL DEFAULT 0,
  remise			double NOT NULL DEFAULT 0,
  unitprice			double(24,8) DEFAULT 0,
  fk_user_creat 	integer,
  fk_user_modif 	integer,
  fk_multicurrency		integer,
  multicurrency_code	varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_price	double(24,8) DEFAULT NULL,
  multicurrency_price_ttc	double(24,8) DEFAULT NULL,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  import_key    	varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_product_price_extrafields (
	rowid               integer AUTO_INCREMENT PRIMARY KEY,
	tms                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_object           integer NOT NULL,
	import_key          varchar(14)
) ENGINE=InnoDB;
CREATE TABLE llx_product_pricerules
(
    rowid INTEGER PRIMARY KEY NOT NULL AUTO_INCREMENT,
    level INTEGER NOT NULL,
    fk_level INTEGER NOT NULL,
    var_percent REAL NOT NULL,
    var_min_percent REAL NOT NULL
)ENGINE=innodb;
create table llx_product_stock
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_product      integer NOT NULL,
  fk_entrepot     integer NOT NULL,
  reel            real,
  import_key      varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_product_thirdparty
(
    rowid                               integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    fk_product                          integer NOT NULL,
    fk_soc                              integer NOT NULL,
    fk_product_thirdparty_relation_type integer NOT NULL,
    date_start                          datetime,
    date_end                            datetime,
    fk_project                          integer,
    description                         text,
    note_public                         text,
    note_private                        text,
    date_creation                       datetime NOT NULL,
    tms                                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_user_creat                       integer NOT NULL,
    fk_user_modif                       integer,
    last_main_doc                       varchar(255),
    import_key                          varchar(14),
    model_pdf                           varchar(255),
    status                              integer DEFAULT 1 NOT NULL
)ENGINE=innodb;
create table llx_product_warehouse_properties
(
  rowid           		integer AUTO_INCREMENT PRIMARY KEY,
  tms             		timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_product      		integer NOT NULL,
  fk_entrepot     		integer NOT NULL,
  seuil_stock_alerte    float DEFAULT '0',
  desiredstock    		float DEFAULT '0',
  import_key      		varchar(14)
)ENGINE=innodb;
create table llx_projet
(
  rowid            	integer AUTO_INCREMENT PRIMARY KEY,
  fk_project        integer DEFAULT NULL,
  fk_soc           	integer,
  datec            	datetime,
  tms              	timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  dateo            	date,
  datee            	date,
  ref              	varchar(50),
  ref_ext          	varchar(50),
  entity           	integer DEFAULT 1 NOT NULL,
  title            	varchar(255) NOT NULL,
  description      	text,
  fk_user_creat    	integer NOT NULL,
  fk_user_modif    	integer,
  public           	integer,
  fk_statut        	integer DEFAULT 0 NOT NULL,
  fk_opp_status    	integer DEFAULT NULL,
  opp_percent	   	double(5,2),
  fk_opp_status_end	integer DEFAULT NULL,
  date_close       	datetime DEFAULT NULL,
  fk_user_close    	integer DEFAULT NULL,
  note_private     	text,
  note_public      	text,
  email_msgid      	varchar(175),
  email_date 		datetime,
  opp_amount       	double(24,8),
  budget_amount    	double(24,8),
  usage_opportunity    integer DEFAULT 0,
  usage_task           integer DEFAULT 1,
  usage_bill_time      integer DEFAULT 0,
  usage_organize_event integer DEFAULT 0,
  date_start_event 	datetime,
  date_end_event   	datetime,
  location         	varchar(255),
  accept_conference_suggestions integer DEFAULT 0,
  accept_booth_suggestions integer DEFAULT 0,
  max_attendees     integer DEFAULT 0,
  price_registration    double(24,8),
  price_booth    	double(24,8),
  model_pdf        	varchar(255),
  ip            	varchar(250),
  last_main_doc	   	varchar(255),
  import_key	   	varchar(14),
  extraparams		varchar(255)
)ENGINE=innodb;
create table llx_projet_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_projet_task
(
  rowid					integer AUTO_INCREMENT PRIMARY KEY,
  ref 					varchar(50),
  entity				integer DEFAULT 1 NOT NULL,
  fk_projet				integer NOT NULL,
  fk_task_parent		integer DEFAULT 0 NOT NULL,
  datec					datetime,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  dateo					datetime,
  datee					datetime,
  datev					datetime,
  label					varchar(255) NOT NULL,
  description			text,
  duration_effective	real DEFAULT 0,
  planned_workload		real DEFAULT 0,
  progress				integer DEFAULT 0,
  priority				integer DEFAULT 0,
  budget_amount         double(24,8),
  fk_user_creat			integer,
  fk_user_modif			integer,
  fk_user_valid			integer,
  fk_statut				smallint DEFAULT 0 NOT NULL,
  note_private			text,
  note_public			text,
  rang                  integer DEFAULT 0,
  model_pdf        		varchar(255),
  import_key			varchar(14),
  billable    smallint DEFAULT 1
)ENGINE=innodb;
create table llx_projet_task_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_propal
(
  rowid					integer AUTO_INCREMENT PRIMARY KEY,
  ref					varchar(30) NOT NULL,
  entity				integer DEFAULT 1 NOT NULL,
  ref_ext				varchar(255),
  ref_client			varchar(255),
  fk_soc				integer,
  fk_projet				integer     DEFAULT NULL,
  tms					timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec					datetime,
  datep					date,
  fin_validite			datetime,
  date_valid			datetime,
  date_signature		datetime,
  date_cloture			datetime,
  fk_user_author		integer,
  fk_user_modif         integer,
  fk_user_valid			integer,
  fk_user_signature		integer,
  fk_user_cloture		integer,
  fk_statut				smallint DEFAULT 0 NOT NULL,
  price					real         DEFAULT 0,
  remise_percent		real         DEFAULT 0,
  remise_absolue		real         DEFAULT 0,
  remise				real         DEFAULT 0,
  total_ht				double(24,8) DEFAULT 0,
  total_tva             double(24,8) DEFAULT 0,
  localtax1				double(24,8) DEFAULT 0,
  localtax2				double(24,8) DEFAULT 0,
  total_ttc				double(24,8) DEFAULT 0,
  fk_account			integer,
  fk_currency			varchar(3),
  fk_cond_reglement		integer,
  deposit_percent		varchar(63) DEFAULT NULL,
  fk_mode_reglement		integer,
  online_sign_ip		varchar(48),
  online_sign_name		varchar(64),
  note_private			text,
  note_public			text,
  model_pdf				varchar(255),
  model_pdf_pos_sign 	varchar(32),
  last_main_doc			varchar(255),
  date_livraison		date DEFAULT NULL,
  fk_shipping_method    integer,
  fk_warehouse		    integer DEFAULT NULL,
  fk_availability		integer NULL,
  fk_input_reason		integer,
  fk_incoterms          integer,
  location_incoterms    varchar(255),
  import_key			varchar(14),
  extraparams			varchar(255),
  fk_delivery_address	integer,
  fk_multicurrency			integer,
  multicurrency_code		varchar(3),
  multicurrency_tx			double(24,8) DEFAULT 1,
  multicurrency_total_ht	double(24,8) DEFAULT 0,
  multicurrency_total_tva	double(24,8) DEFAULT 0,
  multicurrency_total_ttc	double(24,8) DEFAULT 0
)ENGINE=innodb;
create table llx_propal_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_propal_merge_pdf_product (
  rowid integer NOT NULL auto_increment PRIMARY KEY,
  fk_product integer NOT NULL,
  file_name varchar(200) NOT NULL,
  lang 	varchar(5) DEFAULT NULL,
  fk_user_author integer DEFAULT NULL,
  fk_user_mod integer NOT NULL,
  datec datetime NOT NULL,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  import_key varchar(14) DEFAULT NULL
) ENGINE=innodb;
create table llx_propaldet
(
    rowid                           integer AUTO_INCREMENT PRIMARY KEY,
    fk_propal                       integer         NOT NULL,
    fk_parent_line					integer         NULL,
    fk_product						integer         NULL,
    label                           varchar(255)    DEFAULT NULL,
    description                     text,
    fk_remise_except                integer         NULL,
    vat_src_code                    varchar(10)     DEFAULT '',
    tva_tx                          double(7,4)     DEFAULT 0,
    localtax1_tx                    double(7,4)     DEFAULT 0,
    localtax1_type                  varchar(10)	    NULL,
    localtax2_tx               		double(7,4)     DEFAULT 0,
    localtax2_type			 		varchar(10)     NULL,
    qty								real,
    remise_percent					real            DEFAULT 0,
    remise							real            DEFAULT 0,
    price                           real,
    subprice                        double(24,8)    DEFAULT 0,
    subprice_ttc    	  			double(24,8) 	DEFAULT 0,
    total_ht                        double(24,8)    DEFAULT 0,
    total_tva                       double(24,8)    DEFAULT 0,
    total_localtax1					double(24,8)    DEFAULT 0,
    total_localtax2					double(24,8)    DEFAULT 0,
    total_ttc                       double(24,8)    DEFAULT 0,
    product_type                    integer         DEFAULT 0,
    date_start						datetime        DEFAULT NULL,
    date_end                        datetime        DEFAULT NULL,
    info_bits                       integer         DEFAULT 0,
    buy_price_ht					double(24,8)    DEFAULT 0,
    fk_product_fournisseur_price	integer         DEFAULT NULL,
    special_code					integer         DEFAULT 0,
    rang							integer         DEFAULT 0,
    fk_unit                         integer         DEFAULT NULL,
    fk_multicurrency                integer,
    multicurrency_code              varchar(3),
    multicurrency_subprice          double(24,8)    DEFAULT 0,
	multicurrency_subprice_ttc		double(24,8) 	DEFAULT 0,
    multicurrency_total_ht          double(24,8)    DEFAULT 0,
    multicurrency_total_tva         double(24,8)    DEFAULT 0,
    multicurrency_total_ttc         double(24,8)    DEFAULT 0,
    import_key                      varchar(14),
	extraparams				 		varchar(255)
)ENGINE=innodb;
create table llx_propaldet_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_quickmemo_memo(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    entity integer DEFAULT 1 NOT NULL,
    fk_element integer NOT NULL,
    element_type varchar(64) NOT NULL,
    quick_note text,
	date_creation datetime NOT NULL,
    tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	fk_user_archived integer,
    date_archived timestamp DEFAULT NULL,
	import_key varchar(14),
    pos_z integer NOT NULL DEFAULT 0,
    pos_y integer NOT NULL DEFAULT 0,
    pos_x integer NOT NULL DEFAULT 0,
    pos_w integer NOT NULL DEFAULT 0,
    pos_h integer NOT NULL DEFAULT 0,
    color varchar(10),
    context_tab varchar(64),
    private tinyint NOT NULL DEFAULT 1,
    private_tpl tinyint NULL DEFAULT 0,
    rank_tpl integer DEFAULT 0,
    name_tpl varchar(256),
    shared_on_element integer NOT NULL DEFAULT 0,
    status integer NOT NULL DEFAULT 1
) ENGINE=innodb;
create table llx_quickmemo_memo_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_quickmemo_memo_user(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    fk_memo integer NOT NULL,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user integer NOT NULL,
    pos_z integer NOT NULL DEFAULT 0,
    pos_y integer NOT NULL DEFAULT 0,
    pos_x integer NOT NULL DEFAULT 0,
    pos_w integer NOT NULL DEFAULT 0,
    pos_h integer NOT NULL DEFAULT 0
) ENGINE=innodb;
create table llx_reception
(
  rowid                 integer AUTO_INCREMENT PRIMARY KEY,
  tms                   timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  ref                   varchar(30)        NOT NULL,
  entity                integer  DEFAULT 1 NOT NULL,
  fk_soc                integer            NOT NULL,
  fk_projet             integer  DEFAULT NULL,
  ref_ext               varchar(30),
  ref_supplier          varchar(255),
  date_creation         datetime,
  fk_user_author        integer,
  fk_user_modif         integer,
  date_valid            datetime,
  fk_user_valid         integer,
  date_delivery			datetime	DEFAULT NULL,
  date_reception        datetime,
  fk_shipping_method    integer,
  tracking_number       varchar(50),
  fk_statut             smallint	DEFAULT 0,
  billed                smallint    DEFAULT 0,
  height                float,
  width                 float,
  size_units            integer,
  size                  float,
  weight_units          integer,
  weight                float,
  note_private          text,
  note_public           text,
  model_pdf             varchar(255),
  fk_incoterms          integer,
  location_incoterms    varchar(255),
  import_key			varchar(14),
  extraparams			varchar(255)
)ENGINE=innodb;
create table llx_reception_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_receptiondet_batch
(
  rowid          integer AUTO_INCREMENT PRIMARY KEY,
  fk_reception 	 integer  DEFAULT NULL,
  fk_element	 integer,
  fk_elementdet  integer,
  element_type   varchar(50) DEFAULT 'supplier_order' NOT NULL,
  fk_product     integer,
  description	 text,
  qty            float,
  fk_unit		 integer,
  fk_entrepot    integer,
  fk_projet  	 integer  DEFAULT NULL,
  comment		 varchar(255),
  batch          varchar(128) DEFAULT NULL,
  eatby          date DEFAULT NULL,
  sellby         date DEFAULT NULL,
  status         integer,
  fk_user        integer,
  datec          datetime,
  tms            timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  cost_price     double(24,8) DEFAULT 0,
  rang 			 integer  DEFAULT 0,
  extraparams	 varchar(255)
)ENGINE=innodb;
create table llx_receptiondet_batch_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_recruitment_recruitmentcandidature(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity integer NOT NULL DEFAULT 1,
	ref varchar(128) DEFAULT '(PROV)' NOT NULL,
	fk_recruitmentjobposition INTEGER NULL,
	description text,
	note_public text,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer,
	fk_user_modif integer,
	fk_user integer,
	import_key varchar(14),
	model_pdf varchar(255),
	status smallint NOT NULL,
	firstname varchar(128),
	lastname varchar(128),
	email varchar(255),
	phone varchar(64),
	date_birth date,
	remuneration_requested integer,
	remuneration_proposed integer,
	email_msgid varchar(175),
	email_date datetime,
	fk_recruitment_origin INTEGER NULL,
  	ip varchar(250)
) ENGINE=innodb;
create table llx_recruitment_recruitmentcandidature_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_recruitment_recruitmentjobposition(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	ref varchar(128) DEFAULT '(PROV)' NOT NULL,
	entity INTEGER DEFAULT 1 NOT NULL,
	label varchar(255) NOT NULL,
	qty integer DEFAULT 1 NOT NULL,
	fk_soc integer,
	fk_project integer,
	fk_user_recruiter integer,
	email_recruiter varchar(255),
	fk_user_supervisor integer,
	fk_establishment integer,
	date_planned date,
	remuneration_suggested varchar(255),
	description text,
	note_public text,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	last_main_doc varchar(255),
	import_key varchar(14),
	model_pdf varchar(255),
	status smallint NOT NULL
) ENGINE=innodb;
create table llx_recruitment_recruitmentjobposition_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_resource
(
  rowid           		  integer AUTO_INCREMENT PRIMARY KEY,
  entity          		  integer DEFAULT 1 NOT NULL,
  ref             		  varchar(255),
  asset_number          varchar(255),
  description     		  text,
  fk_code_type_resource varchar(32),
  address               varchar(255)    DEFAULT NULL,
  zip                   varchar(25)     DEFAULT NULL,
  town                  varchar(50)     DEFAULT NULL,
  photo_filename        varchar(255)    DEFAULT NULL,
  max_users             integer         DEFAULT NULL,
  phone                 varchar(30)     DEFAULT NULL,
  email                 varchar(255)    DEFAULT NULL,
  url                   varchar(255)    DEFAULT NULL,
  datec                 datetime DEFAULT NULL,
  date_valid            datetime DEFAULT NULL,
  fk_user_author        integer DEFAULT NULL,
  fk_user_modif         integer DEFAULT NULL,
  fk_user_valid         integer DEFAULT NULL,
  fk_statut             smallint NOT NULL DEFAULT '0',
  note_public     		  text,
  note_private    		  text,
  import_key			varchar(14),
  extraparams			varchar(255),
  fk_country          integer DEFAULT NULL,
  fk_state              integer DEFAULT NULL,
  tms         			  timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
)ENGINE=innodb;
create table llx_resource_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_rights_def
(
  id				integer NOT NULL,
  entity			integer DEFAULT 1 NOT NULL,
  libelle			varchar(255),
  module			varchar(64),
  module_origin		varchar(64),
  module_position	integer DEFAULT 0 NOT NULL,
  family            varchar(64) NULL,
  family_position	integer DEFAULT 0 NOT NULL,
  perms				varchar(50),
  subperms			varchar(50),
  type				varchar(1),
  bydefault			tinyint DEFAULT 0,
  enabled			text NULL
)ENGINE=innodb;
create table llx_salary
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  ref             varchar(30) NULL,
  ref_ext         varchar(255),
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec           datetime,
  fk_user         integer NOT NULL,
  datep           date,
  datev           date,
  salary          double(24,8),
  amount          double(24,8) NOT NULL DEFAULT 0,
  fk_projet       integer DEFAULT NULL,
  fk_typepayment  integer NOT NULL,
  num_payment     varchar(50),
  label           varchar(255),
  datesp          date,
  dateep          date,
  entity          integer DEFAULT 1 NOT NULL,
  note            text,
  note_public     text,
  model_pdf 			varchar(255) DEFAULT NULL,
  fk_bank         integer,
  paye            smallint default 0 NOT NULL,
  fk_account      integer,
  fk_user_author  integer,
  fk_user_modif   integer
)ENGINE=innodb;
create table llx_salary_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_session
(
  session_id varchar(50) PRIMARY KEY,
  session_variable text,
  date_creation datetime NOT NULL,
  last_accessed datetime NOT NULL,
  fk_user integer NOT NULL,
  remote_ip varchar(64) NULL,
  user_agent varchar(255) NULL
)ENGINE=innodb;
create table llx_societe
(
  rowid                    integer AUTO_INCREMENT PRIMARY KEY,
  nom                      varchar(128),
  name_alias               varchar(128) NULL,
  entity                   integer DEFAULT 1 NOT NULL,
  ref_ext                  varchar(255),
  statut                   tinyint        DEFAULT 0,
  parent                   integer,
  status            	   tinyint 		  DEFAULT 1,
  code_client              varchar(24),
  code_fournisseur         varchar(24),
  tp_payment_reference     varchar(25),
  accountancy_code_customer_general	varchar(32) DEFAULT NULL,
  code_compta              			varchar(32),
  accountancy_code_supplier_general varchar(32) DEFAULT NULL,
  code_compta_fournisseur  			varchar(32),
  address                  varchar(255),
  zip                      varchar(25),
  town                     varchar(50),
  fk_departement           integer        DEFAULT 0,
  fk_pays                  integer        DEFAULT 0,
  geolat                   double(24,8)   DEFAULT NULL,
  geolong                  double(24,8)   DEFAULT NULL,
  geopoint                 point DEFAULT NULL,
  georesultcode            varchar(16),
  phone                    varchar(30),
  phone_mobile             varchar(30),
  fax                      varchar(30),
  url                      varchar(255),
  email                    varchar(128),
  fk_account               integer        DEFAULT 0,
  socialnetworks           text DEFAULT NULL,
  fk_effectif              integer        DEFAULT 0,
  fk_typent                integer        DEFAULT NULL,
  fk_forme_juridique       integer        DEFAULT 0,
  birth                    date,
  fk_currency			   varchar(3),
  siren	                   varchar(128),
  siret                    varchar(128),
  ape                      varchar(128),
  idprof4                  varchar(128),
  idprof5                  varchar(128),
  idprof6                  varchar(128),
  euid                     varchar(64),
  tva_intra                varchar(20),
  capital                  double(24,8)   DEFAULT NULL,
  fk_stcomm                integer        DEFAULT 0 NOT NULL,
  note_private             text,
  note_public              text,
  model_pdf				   varchar(255),
  last_main_doc			   varchar(255),
  prefix_comm              varchar(5),
  client                   tinyint        DEFAULT 0,
  fournisseur              tinyint        DEFAULT 0,
  supplier_account         varchar(32),
  fk_prospectlevel         varchar(12),
  fk_incoterms             integer,
  location_incoterms       varchar(255),
  customer_bad             tinyint        DEFAULT 0,
  customer_rate            real           DEFAULT 0,
  supplier_rate            real           DEFAULT 0,
  remise_client            real           DEFAULT 0,
  remise_supplier          real           DEFAULT 0,
  mode_reglement           integer,
  cond_reglement           tinyint,
  deposit_percent          varchar(63) DEFAULT NULL,
  transport_mode           tinyint,
  mode_reglement_supplier  tinyint,
  cond_reglement_supplier  tinyint,
  transport_mode_supplier  tinyint,
  fk_shipping_method       integer,
  tva_assuj                tinyint        DEFAULT 1,
  vatexemptcode            varchar(24),
  vat_reverse_charge       tinyint        DEFAULT 0,
  localtax1_assuj          tinyint        DEFAULT 0,
  localtax1_value 		   double(7,4),
  localtax2_assuj          tinyint        DEFAULT 0,
  localtax2_value 		   double(7,4),
  barcode                  varchar(180),
  fk_barcode_type          integer NULL   DEFAULT 0,
  price_level              integer NULL,
  outstanding_limit	       double(24,8)   DEFAULT NULL,
  order_min_amount	       double(24,8)   DEFAULT NULL,
  supplier_order_min_amount	       double(24,8)   DEFAULT NULL,
  default_lang             varchar(6),
  logo                     varchar(255)   DEFAULT NULL,
  logo_squarred            varchar(255)   DEFAULT NULL,
  canvas				   varchar(32)    DEFAULT NULL,
  fk_warehouse			   integer 		  DEFAULT NULL,
  webservices_url          varchar(255),
  webservices_key          varchar(128),
  accountancy_code_sell         varchar(32),
  accountancy_code_buy          varchar(32),
  datec	                   datetime,
  tms                      timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat            integer NULL,
  fk_user_modif            integer,
  fk_multicurrency		   integer,
  multicurrency_code	   varchar(3),
  ip                       varchar(250),
  import_key               varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_societe_account(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity	integer DEFAULT 1,
	login             varchar(128) NOT NULL,
	pass_encoding     varchar(24),
	pass_crypted      varchar(128),
	pass_temp         varchar(128),
	fk_soc            integer,
	fk_website        integer,
	site              varchar(128) NOT NULL,
	site_account      varchar(128),
	key_account       varchar(128),
	note_private      text,
	date_last_login   datetime,
	date_previous_login datetime,
	date_last_reset_password datetime,
	date_creation     datetime NOT NULL,
	tms               timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat     integer NOT NULL,
	fk_user_modif     integer,
	ip                varchar(250),
	import_key        varchar(14),
	status            integer
) ENGINE=innodb;
create table llx_societe_commerciaux
(
  rowid         integer AUTO_INCREMENT PRIMARY KEY,
  fk_soc        integer,
  fk_user       integer,
  fk_c_type_contact_code varchar(32) NOT NULL DEFAULT 'SALESREPTHIRD',
  import_key	varchar(14)
)ENGINE=innodb;
create table llx_societe_contacts
(
    rowid           integer AUTO_INCREMENT PRIMARY KEY,
    entity          integer DEFAULT 1 NOT NULL,
    date_creation           datetime NOT NULL,
    fk_soc		        integer NOT NULL,
    fk_c_type_contact	int NOT NULL,
    fk_socpeople        integer NOT NULL,
    tms TIMESTAMP,
    import_key VARCHAR(14)
)ENGINE=innodb;
create table llx_societe_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_societe_perentity
(
  	rowid								integer AUTO_INCREMENT PRIMARY KEY,
  	fk_soc								integer,
  	entity								integer DEFAULT 1 NOT NULL,
	accountancy_code_customer_general	varchar(32) DEFAULT NULL,
  	accountancy_code_customer			varchar(32),
	accountancy_code_supplier_general	varchar(32) DEFAULT NULL,
  	accountancy_code_supplier			varchar(32),
  	accountancy_code_sell				varchar(32),
  	accountancy_code_buy				varchar(32),
	vat_reverse_charge					tinyint DEFAULT 0,
	fk_account							integer DEFAULT NULL,
	mode_reglement						integer DEFAULT NULL,
	cond_reglement						tinyint DEFAULT NULL,
	mode_reglement_supplier				tinyint DEFAULT NULL,
	cond_reglement_supplier				tinyint DEFAULT NULL
)ENGINE=innodb;
create table llx_societe_prices
(
  rowid              integer AUTO_INCREMENT PRIMARY KEY,
  fk_soc             integer   DEFAULT 0,
  tms                timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec	             datetime,
  fk_user_author     integer,
  price_level        tinyint   DEFAULT 1
)ENGINE=innodb;
create table llx_societe_remise
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  entity			integer DEFAULT 1 NOT NULL,
  fk_soc			integer NOT NULL,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec				datetime,
  fk_user_author	integer,
  remise_client		double(7,4)  DEFAULT 0 NOT NULL,
  note				text
)ENGINE=innodb;
create table llx_societe_remise_except
(
  rowid						integer AUTO_INCREMENT PRIMARY KEY,
  entity					integer DEFAULT 1 NOT NULL,
  fk_soc					integer NOT NULL,
  discount_type				integer DEFAULT 0 NOT NULL,
  datec						datetime,
  amount_ht					double(24,8) NOT NULL,
  amount_tva				double(24,8) DEFAULT 0 NOT NULL,
  amount_localtax1	double(24,8) DEFAULT 0 NOT NULL,
  amount_localtax2	double(24,8) DEFAULT 0 NOT NULL,
  amount_ttc				double(24,8) DEFAULT 0 NOT NULL,
  tva_tx					double(7,4)  DEFAULT 0 NOT NULL,
  localtax1_tx			double(7,4)  DEFAULT 0 NOT NULL,
  localtax1_type    varchar(10)  NULL,
  localtax2_tx					double(7,4)  DEFAULT 0 NOT NULL,
  localtax2_type    varchar(10)  NULL,
  vat_src_code				varchar(10)  DEFAULT '',
  fk_user					integer NOT NULL,
  fk_facture_line			integer,
  fk_facture				integer,
  fk_facture_source			integer,
  fk_invoice_supplier_line	integer,
  fk_invoice_supplier		integer,
  fk_invoice_supplier_source integer,
  description				text NOT NULL,
  multicurrency_code		varchar(3) NULL,
  multicurrency_tx			double(24,8) NULL,
  multicurrency_amount_ht	double(24,8) DEFAULT 0 NOT NULL,
  multicurrency_amount_tva	double(24,8) DEFAULT 0 NOT NULL,
  multicurrency_amount_ttc	double(24,8) DEFAULT 0 NOT NULL
)ENGINE=innodb;
create table llx_societe_remise_supplier
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  entity			integer DEFAULT 1 NOT NULL,
  fk_soc			integer NOT NULL,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec				datetime,
  fk_user_author	integer,
  remise_supplier	double(7,4)  DEFAULT 0 NOT NULL,
  note				text
)ENGINE=innodb;
create table llx_societe_rib
(
  rowid								integer AUTO_INCREMENT PRIMARY KEY,
  entity							integer DEFAULT 1 NOT NULL,
  type								varchar(32) DEFAULT 'ban' NOT NULL,
  label								varchar(180),
  fk_soc							integer NOT NULL,
  datec								datetime,
  tms								timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  bank								varchar(255),
  code_banque						varchar(128),
  code_guichet						varchar(6),
  number							varchar(255),
  cle_rib							varchar(5),
  bic								varchar(20),
  bic_intermediate					varchar(11),
  iban_prefix						varchar(100),
  cci                               varchar(100),
  domiciliation						varchar(255),
  proprio							varchar(60),
  owner_address						varchar(255),
  default_rib						smallint NOT NULL DEFAULT 0,
  state_id							integer,
  fk_country						integer,
  currency_code						varchar(3),
  model_pdf							varchar(255),
  last_main_doc						varchar(255),
  rum								varchar(32),
  date_rum							date,
  frstrecur							varchar(16) DEFAULT 'FRST',
  last_four							varchar(4),
  card_type							varchar(255),
  cvn								varchar(255),
  exp_date_month					integer,
  exp_date_year						integer,
  country_code						varchar(10),
  approved							integer DEFAULT 0,
  email 							varchar(255),
  ending_date						date,
  max_total_amount_of_all_payments	double(24,8),
  preapproval_key					varchar(255),
  starting_date						date,
  total_amount_of_all_payments		double(24,8),
  stripe_card_ref					varchar(128),
  stripe_account					varchar(128),
  ext_payment_site					varchar(128),
  extraparams						varchar(255),
  date_signature					datetime,
  online_sign_ip					varchar(48),
  online_sign_name					varchar(64),
  comment							varchar(255),
  ipaddress							varchar(68),
  status							integer NOT NULL DEFAULT 1,
  import_key						varchar(14)
)ENGINE=innodb;
create table llx_socpeople
(
  rowid				integer AUTO_INCREMENT PRIMARY KEY,
  datec				datetime,
  tms				timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_soc			integer,
  entity			integer DEFAULT 1 NOT NULL,
  ref_ext           varchar(255),
  name_alias        varchar(255),
  fk_parent         integer NULL,
  civility			varchar(6),
  lastname			varchar(50),
  firstname			varchar(50),
  address			varchar(255),
  zip				varchar(25),
  town				varchar(255),
  fk_departement	integer,
  fk_pays			integer        DEFAULT 0,
  geolat                   double(24,8)   DEFAULT NULL,
  geolong                  double(24,8)   DEFAULT NULL,
  geopoint                 point DEFAULT NULL,
  georesultcode            varchar(16),
  birthday			date,
  poste				varchar(255),
  phone				varchar(30),
  phone_perso		varchar(30),
  phone_mobile		varchar(30),
  fax				varchar(30),
  url               varchar(255),
  email				varchar(255),
  socialnetworks    text DEFAULT NULL,
  photo				varchar(255),
  no_email			smallint NOT NULL DEFAULT 0,
  priv				smallint NOT NULL DEFAULT 0,
  fk_prospectlevel  	varchar(12),
  fk_stcommcontact  		integer        DEFAULT 0 NOT NULL,
  fk_user_creat		integer DEFAULT 0,
  fk_user_modif		integer,
  note_private		text,
  note_public		text,
  default_lang		varchar(6),
  canvas			varchar(32),
  import_key		varchar(14),
  statut			tinyint DEFAULT 1 NOT NULL,
  ip    varchar(250)
)ENGINE=innodb;
create table llx_socpeople_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_stock_mouvement
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datem           datetime,
  fk_product      integer NOT NULL,
  batch           varchar(128) DEFAULT NULL,
  eatby           date DEFAULT NULL,
  sellby          date DEFAULT NULL,
  fk_entrepot     integer NOT NULL,
  value			  real,
  price           double(24,8) DEFAULT 0,
  type_mouvement  smallint,
  fk_user_author  integer,
  label           varchar(255),
  inventorycode   varchar(128),
  fk_project	  integer,
  fk_origin       integer,
  origintype      varchar(64),
  model_pdf       varchar(255),
  fk_projet       integer NOT NULL DEFAULT 0
)ENGINE=innodb;
CREATE TABLE llx_stock_mouvement_extrafields (
    rowid integer AUTO_INCREMENT PRIMARY KEY,
    tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object integer NOT NULL,
    import_key varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_stocktransfer_stocktransfer(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
    entity integer  DEFAULT 1 NOT NULL,
	ref varchar(128) DEFAULT '(PROV)' NOT NULL,
	label varchar(255),
	fk_soc integer,
	fk_project integer,
    fk_warehouse_source integer,
    fk_warehouse_destination integer,
    description text,
	note_public text,
	note_private text,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    date_creation datetime NOT NULL,
    date_prevue_depart date DEFAULT NULL,
    date_reelle_depart date DEFAULT NULL,
    date_prevue_arrivee date DEFAULT NULL,
    date_reelle_arrivee date DEFAULT NULL,
    lead_time_for_warning integer DEFAULT NULL,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	import_key varchar(14),
	model_pdf varchar(255),
	last_main_doc varchar(255),
	status smallint NOT NULL,
	fk_incoterms          integer,
    location_incoterms    varchar(255)
) ENGINE=innodb;
create table llx_stocktransfer_stocktransfer_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_stocktransfer_stocktransferline(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	amount double DEFAULT NULL,
	qty real,
    fk_warehouse_source integer NOT NULL,
    fk_warehouse_destination integer NOT NULL,
	fk_stocktransfer integer NOT NULL,
	fk_product integer NOT NULL,
    batch varchar(128) DEFAULT NULL,
    pmp double,
    rang integer DEFAULT 0,
    fk_parent_line integer NULL
) ENGINE=innodb;
create table llx_stocktransfer_stocktransferline_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
create table llx_subscription
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec           datetime,
  fk_adherent     integer,
  fk_type         integer,
  dateadh         datetime,
  datef           datetime,
  subscription    double(24,8),
  fk_bank         integer DEFAULT NULL,
  fk_user_creat   integer DEFAULT NULL,
  fk_user_valid   integer DEFAULT NULL,
  ref_ext 		  varchar(128),
  note            text,
  note_private    text,
  import_key       varchar(14)
)ENGINE=innodb;
CREATE TABLE llx_supplier_proposal (
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  ref varchar(30) NOT NULL,
  entity integer NOT NULL DEFAULT 1,
  ref_ext varchar(255) DEFAULT NULL,
  fk_soc integer DEFAULT NULL,
  fk_projet integer DEFAULT NULL,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec datetime DEFAULT NULL,
  date_valid datetime DEFAULT NULL,
  date_cloture datetime DEFAULT NULL,
  fk_user_author integer DEFAULT NULL,
  fk_user_modif integer DEFAULT NULL,
  fk_user_valid integer DEFAULT NULL,
  fk_user_cloture integer DEFAULT NULL,
  fk_statut	smallint DEFAULT 0 NOT NULL,
  price double DEFAULT 0,
  remise_percent double DEFAULT 0,
  remise_absolue double DEFAULT 0,
  remise double DEFAULT 0,
  total_ht double(24,8) DEFAULT 0,
  total_tva double(24,8) DEFAULT 0,
  localtax1 double(24,8) DEFAULT 0,
  localtax2 double(24,8) DEFAULT 0,
  total_ttc double(24,8) DEFAULT 0,
  fk_account integer DEFAULT NULL,
  fk_currency varchar(3) DEFAULT NULL,
  fk_cond_reglement integer DEFAULT NULL,
  deposit_percent varchar(63) DEFAULT NULL,
  fk_mode_reglement integer DEFAULT NULL,
  note_private text,
  note_public text,
  model_pdf 			varchar(255) DEFAULT NULL,
  last_main_doc			varchar(255),
  date_livraison date DEFAULT NULL,
  fk_shipping_method integer DEFAULT NULL,
  import_key varchar(14) DEFAULT NULL,
  extraparams varchar(255) DEFAULT NULL,
  fk_multicurrency        integer,
  multicurrency_code      varchar(3),
  multicurrency_tx        double(24,8) DEFAULT 1,
  multicurrency_total_ht  double(24,8) DEFAULT 0,
  multicurrency_total_tva double(24,8) DEFAULT 0,
  multicurrency_total_ttc double(24,8) DEFAULT 0
) ENGINE=innodb;
CREATE TABLE llx_supplier_proposal_extrafields (
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object integer NOT NULL,
  import_key varchar(14) DEFAULT NULL
) ENGINE=innodb;
CREATE TABLE llx_supplier_proposaldet (
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  fk_supplier_proposal integer NOT NULL,
  fk_parent_line integer DEFAULT NULL,
  fk_product integer DEFAULT NULL,
  label varchar(255) DEFAULT NULL,
  description text,
  fk_remise_except integer DEFAULT NULL,
  vat_src_code					varchar(10) DEFAULT '',
  tva_tx 						double(7,4) DEFAULT 0,
  localtax1_tx double(7,4) DEFAULT 0,
  localtax1_type varchar(10) DEFAULT NULL,
  localtax2_tx double(7,4) DEFAULT 0,
  localtax2_type varchar(10) DEFAULT NULL,
  qty double DEFAULT NULL,
  remise_percent double DEFAULT '0',
  remise double DEFAULT '0',
  price double DEFAULT NULL,
  subprice 						double(24,8) DEFAULT 0,
  subprice_ttc      			double(24,8) DEFAULT 0,
  total_ht double(24,8) DEFAULT 0,
  total_tva double(24,8) DEFAULT 0,
  total_localtax1 double(24,8) DEFAULT 0,
  total_localtax2 double(24,8) DEFAULT 0,
  total_ttc double(24,8) DEFAULT 0,
  product_type integer DEFAULT 0,
  date_start	datetime   DEFAULT NULL,
  date_end		datetime   DEFAULT NULL,
  info_bits integer DEFAULT 0,
  buy_price_ht double(24,8) DEFAULT 0,
  fk_product_fournisseur_price integer DEFAULT NULL,
  special_code integer DEFAULT 0,
  rang integer DEFAULT 0,
  ref_fourn varchar(128) DEFAULT NULL,
  fk_multicurrency        integer,
  multicurrency_code      varchar(3),
  multicurrency_subprice  		double(24,8) DEFAULT 0,
  multicurrency_subprice_ttc	double(24,8) DEFAULT 0,
  multicurrency_total_ht  double(24,8) DEFAULT 0,
  multicurrency_total_tva double(24,8) DEFAULT 0,
  multicurrency_total_ttc double(24,8) DEFAULT 0,
  fk_unit integer DEFAULT NULL,
  extraparams	varchar(255)
) ENGINE=innodb;
CREATE TABLE llx_supplier_proposaldet_extrafields (
  rowid integer AUTO_INCREMENT PRIMARY KEY,
  tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object integer NOT NULL,
  import_key varchar(14) DEFAULT NULL
) ENGINE=innodb;
CREATE TABLE llx_takepos_floor_tables(
    rowid integer AUTO_INCREMENT PRIMARY KEY,
    entity integer DEFAULT 1 NOT NULL,
    label varchar(255),
    leftpos float,
    toppos float,
    floor smallint
) ENGINE=innodb;
CREATE TABLE llx_ticket
(
	rowid       integer AUTO_INCREMENT PRIMARY KEY,
	entity		integer DEFAULT 1,
	ref			varchar(128) NOT NULL,
	track_id    varchar(128) NOT NULL,
	fk_soc		integer DEFAULT 0,
	fk_project	integer DEFAULT 0,
	fk_contract	integer DEFAULT 0,
	origin_email   varchar(128),
	origin_replyto   varchar(128),
	origin_references   text,
	fk_user_create	integer,
	fk_user_assign	integer,
	subject	varchar(255),
	message	mediumtext,
	fk_statut integer,
	note_public text,
	note_private text,
	fk_user_modif integer,
	resolution integer,
	progress integer DEFAULT 0,
	timing varchar(20),
	type_code varchar(32),
	category_code varchar(32),
	severity_code varchar(32),
	datec datetime,
	date_read datetime,
	date_last_msg_sent datetime,
	date_close datetime,
	notify_tiers_at_create tinyint,
	email_msgid varchar(255),
	email_date datetime,
	ip varchar(250),
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	model_pdf varchar(255),
	last_main_doc varchar(255),
	extraparams varchar(255),
	barcode varchar(255) DEFAULT NULL,
    fk_barcode_type integer DEFAULT NULL,
    import_key        varchar(14)
)ENGINE=innodb;
create table llx_ticket_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_tva
(
  rowid           integer AUTO_INCREMENT PRIMARY KEY,
  tms             timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  datec           datetime,
  datep           date,
  datev           date,
  amount          double(24,8) NOT NULL DEFAULT 0,
  fk_typepayment  integer NULL,
  num_payment     varchar(50),
  label           varchar(255),
  entity          integer DEFAULT 1 NOT NULL,
  note            text,
  paye            smallint default 0 NOT NULL,
  fk_account      integer,
  fk_user_creat   integer,
  fk_user_modif   integer,
  import_key      varchar(14)
)ENGINE=innodb;
create table llx_user
(
  rowid               integer AUTO_INCREMENT PRIMARY KEY,
  entity              integer     DEFAULT 1 NOT NULL,
  ref_employee        varchar(50),
  ref_ext             varchar(50),
  admin               smallint DEFAULT 0,
  employee            tinyint     DEFAULT 1,
  fk_establishment    integer     DEFAULT 0,
  datec               datetime,
  tms                 timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat       integer,
  fk_user_modif       integer,
  login               varchar(50) NOT NULL,
  pass_encoding       varchar(24),
  pass                varchar(128),
  pass_crypted        varchar(128),
  pass_temp           varchar(128),
  force_pass_change   tinyint       DEFAULT 0,
  api_key             varchar(128),
  gender              varchar(10),
  civility            varchar(6),
  lastname            varchar(50),
  firstname           varchar(50),
  address             varchar(255),
  zip                 varchar(25),
  town                varchar(50),
  fk_state            integer        DEFAULT 0,
  fk_country          integer        DEFAULT 0,
  birth               date,
  birth_place         varchar(64),
  job                 varchar(128),
  office_phone        varchar(30),
  office_fax          varchar(30),
  user_mobile         varchar(30),
  personal_mobile     varchar(30),
  email               varchar(255),
  personal_email      varchar(255),
  email_oauth2        varchar(255),
  signature           longtext DEFAULT NULL,
  socialnetworks      text DEFAULT NULL,
  fk_soc                       integer NULL,
  fk_socpeople                 integer NULL,
  fk_member                    integer NULL,
  fk_user                      integer NULL,
  fk_user_expense_validator    integer NULL,
  fk_user_holiday_validator    integer NULL,
  national_registration_number  varchar(50),
  idpers1			   varchar(128),
  idpers2			   varchar(128),
  idpers3			   varchar(128),
  note_public		      text,
  note_private            text          DEFAULT NULL,
  model_pdf               varchar(255)  DEFAULT NULL,
  last_main_doc           varchar(255),
  datelastlogin           datetime,
  datepreviouslogin       datetime,
  datelastpassvalidation  datetime,
  datestartvalidity       datetime,
  dateendvalidity         datetime,
  flagdelsessionsbefore   datetime DEFAULT NULL,
  iplastlogin             varchar(250),
  ippreviouslogin         varchar(250),
  ldap_sid                varchar(255)  DEFAULT NULL,
  openid                  varchar(255),
  statut                  tinyint       DEFAULT 1,
  photo                   varchar(255),
  lang                    varchar(6),
  color                   varchar(6),
  barcode                 varchar(255)  DEFAULT NULL,
  fk_barcode_type         integer       DEFAULT 0,
  accountancy_code_user_general	varchar(32) DEFAULT NULL,
  accountancy_code				varchar(32) NULL,
  nb_holiday              integer       DEFAULT 0,
  thm                     double(24,8),
  tjm                     double(24,8),
  access_hours			  varchar(128) DEFAULT NULL,
  salary                  double(24,8),
  salaryextra             double(24,8),
  dateemployment          date,
  dateemploymentend       date,
  weeklyhours             double(16,8),
  import_key             varchar(14),
  default_range          integer,
  default_c_exp_tax_cat  integer,
  fk_warehouse           integer
)ENGINE=innodb;
create table llx_user_alert
(
  rowid        integer AUTO_INCREMENT PRIMARY KEY,
  type         integer,
  fk_contact   integer,
  fk_user      integer
)ENGINE=innodb;
create table llx_user_clicktodial
(
  fk_user       integer PRIMARY KEY,
  url           varchar(255),
  login         varchar(32),
  pass          varchar(64),
  poste         varchar(20)
)ENGINE=innodb;
create table llx_user_employment
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  entity            integer DEFAULT 1 NOT NULL,
  ref				varchar(50),
  ref_ext			varchar(50),
  fk_user			integer,
  datec             datetime,
  tms               timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_user_creat     integer,
  fk_user_modif     integer,
  job				varchar(128),
  status            integer NOT NULL,
  salary			double(24,8),
  salaryextra		double(24,8),
  weeklyhours		double(16,8),
  dateemployment    date,
  dateemploymentend date
)ENGINE=innodb;
create table llx_user_extrafields
(
  rowid            integer AUTO_INCREMENT PRIMARY KEY,
  tms              timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object        integer NOT NULL,
  import_key       varchar(14)
)ENGINE=innodb;
create table llx_user_param
(
  fk_user       integer      NOT NULL,
  entity        integer DEFAULT 1 NOT NULL,
  param         varchar(180)  NOT NULL,
  value         text NOT NULL
)ENGINE=innodb;
create table llx_user_rib
(
  rowid             integer AUTO_INCREMENT PRIMARY KEY,
  fk_user           integer      NOT NULL,
  entity            integer DEFAULT 1 NOT NULL,
  datec             datetime,
  tms               timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  label             varchar(30),
  bank              varchar(255),
  code_banque       varchar(128),
  code_guichet      varchar(6),
  number            varchar(255),
  cle_rib           varchar(5),
  bic               varchar(11),
  bic_intermediate  varchar(11),
  iban_prefix       varchar(100),
  domiciliation     varchar(255),
  proprio           varchar(60),
  owner_address     varchar(255),
  state_id          integer,
  fk_country        integer,
  currency_code     varchar(3),
  default_rib       smallint DEFAULT 0 NOT NULL
)ENGINE=innodb;
create table llx_user_rights
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  entity		integer DEFAULT 1 NOT NULL,
  fk_user		integer NOT NULL,
  fk_id			integer NOT NULL
)ENGINE=innodb;
create table llx_usergroup
(
  rowid         integer AUTO_INCREMENT PRIMARY KEY,
  nom           varchar(180) NOT NULL,
  entity        integer DEFAULT 1 NOT NULL,
  fk_parent     integer DEFAULT NULL,
  datec         datetime,
  tms           timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  color         varchar(6),
  note          text,
  model_pdf     varchar(255) DEFAULT NULL
)ENGINE=innodb;
CREATE TABLE llx_usergroup_extrafields (
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
)ENGINE=innodb;
create table llx_usergroup_rights
(
  rowid			integer AUTO_INCREMENT PRIMARY KEY,
  entity		integer DEFAULT 1 NOT NULL,
  fk_usergroup	integer NOT NULL,
  fk_id			integer NOT NULL
)ENGINE=innodb;
create table llx_usergroup_user
(
  rowid         integer AUTO_INCREMENT PRIMARY KEY,
  entity        integer DEFAULT 1 NOT NULL,
  fk_user       integer NOT NULL,
  fk_usergroup  integer NOT NULL
)ENGINE=innodb;
CREATE TABLE llx_webhook_history(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	trigger_code varchar(128) NOT NULL,
	trigger_data text NOT NULL,
	fk_target integer NOT NULL,
	url varchar(255) NOT NULL,
	error_message text,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	import_key varchar(14),
	status integer DEFAULT 1 NOT NULL
) ENGINE=innodb;
CREATE TABLE llx_webhook_target (
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	entity integer DEFAULT 1 NOT NULL,
	ref varchar(128) NOT NULL,
	label varchar(255),
	type integer DEFAULT 0 NOT NULL,
	description text,
	note_public text,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	import_key varchar(14),
	status integer DEFAULT 0 NOT NULL,
	url varchar(255) NOT NULL,
	connection_method varchar(255) NULL,
	connection_data varchar(255) NULL,
	trigger_codes text
) ENGINE=innodb;
CREATE TABLE llx_website
(
	rowid         integer AUTO_INCREMENT NOT NULL PRIMARY KEY,
	type_container varchar(16) NOT NULL DEFAULT 'page',
	entity        integer NOT NULL DEFAULT 1,
	ref	          varchar(128) NOT NULL,
	description   varchar(255),
	maincolor     varchar(16),
	maincolorbis  varchar(16),
	lang          varchar(8),
	otherlang     varchar(255),
	status		  integer DEFAULT 1,
	fk_default_home integer,
	use_manifest integer,
	virtualhost   varchar(255),
    fk_user_creat integer,
    fk_user_modif integer,
    date_creation datetime,
    position      integer DEFAULT 0,
    paymentframemode integer DEFAULT 0,
    lastaccess    datetime NULL,
    lastpageid    integer DEFAULT 0,
    pageviews_previous_month BIGINT UNSIGNED DEFAULT 0,
    pageviews_month BIGINT UNSIGNED DEFAULT 0,
    pageviews_total BIGINT UNSIGNED DEFAULT 0,
	tms           timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    import_key    varchar(14),
	name_template varchar(255) NULL
) ENGINE=innodb;
CREATE TABLE llx_website_extrafields
(
  rowid                     integer AUTO_INCREMENT PRIMARY KEY,
  tms                       timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  fk_object                 integer NOT NULL,
  import_key                varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_website_page
(
	rowid         integer AUTO_INCREMENT NOT NULL PRIMARY KEY,
	fk_website    integer NOT NULL,
	type_container varchar(16) NOT NULL DEFAULT 'page',
	pageurl       varchar(255) NOT NULL,
	aliasalt      varchar(255),
	title         varchar(255),
	description   varchar(255),
	image         varchar(255),
	keywords      varchar(255),
	metarobots    varchar(128),
	lang          varchar(6),
	fk_page       integer,
	allowed_in_frames  integer DEFAULT 0,
	htmlheader	  text,
	content		  mediumtext,
    status        integer DEFAULT 1,
	grabbed_from  varchar(255),
    fk_user_creat integer,
    fk_user_modif integer,
    author_alias  varchar(64),
    date_creation datetime,
	tms           timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    import_key    varchar(14),
    object_type   varchar(255),
    fk_object     varchar(255),
    keep_history  integer DEFAULT 5
) ENGINE=innodb;
CREATE TABLE llx_workstation_workstation(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	ref varchar(128) DEFAULT '(PROV)' NOT NULL,
    label varchar(255),
    type varchar(7),
    note_public text,
	entity int DEFAULT 1,
	note_private text,
	date_creation datetime NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_user_creat integer NOT NULL,
	fk_user_modif integer,
	import_key varchar(14),
	status smallint NOT NULL,
	nb_operators_required integer,
	thm_operator_estimated double,
	thm_machine_estimated double
) ENGINE=innodb;
create table llx_workstation_workstation_extrafields
(
    rowid           integer     AUTO_INCREMENT PRIMARY KEY,
    tms             timestamp   DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    fk_object       integer     NOT NULL,
    import_key      varchar(14)
) ENGINE=innodb;
CREATE TABLE llx_workstation_workstation_resource(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_resource integer,
	fk_workstation integer
) ENGINE=innodb;
CREATE TABLE llx_workstation_workstation_usergroup(
	rowid integer AUTO_INCREMENT PRIMARY KEY NOT NULL,
	tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	fk_usergroup integer,
	fk_workstation integer
) ENGINE=innodb;
CREATE TABLE llx_zapier_hook(
    rowid integer AUTO_INCREMENT PRIMARY KEY,
    entity integer DEFAULT 1 NOT NULL,
    url varchar(255),
    event varchar(255),
    module varchar(128),
    action varchar(128),
    status integer,
    date_creation datetime NOT NULL,
    fk_user integer NOT NULL,
    tms timestamp DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    import_key varchar(14)
) ENGINE=innodb;
