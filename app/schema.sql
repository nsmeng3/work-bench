-- Add new schema named "db_wenku"
CREATE SCHEMA "db_wenku";
-- Add new schema named "public"
CREATE SCHEMA IF NOT EXISTS "public";
-- Set comment to schema: "public"
COMMENT ON SCHEMA "public" IS 'standard public schema';
-- Create "jpaeventpublication" table
CREATE TABLE "db_wenku"."jpaeventpublication" (
    "id" uuid NOT NULL,
    "completiondate" timestamp NULL,
    "eventtype" character varying(255) NULL,
    "listenerid" character varying(255) NULL,
    "publicationdate" timestamp NULL,
    "serializedevent" character varying(255) NULL,
    PRIMARY KEY ("id")
);
-- Create "t_advertisement" table
CREATE TABLE "db_wenku"."t_advertisement" (
    "c_id" character varying(50) NOT NULL,
    "c_key" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "c_value" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_ai_feedback" table
CREATE TABLE "db_wenku"."t_ai_feedback" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_groupid" character varying(50) NOT NULL,
    "c_type" character varying(100) NOT NULL,
    "c_input" text NULL,
    "c_gxlx" character varying(100) NULL,
    "c_xznr" text NULL,
    "c_prompt" text NOT NULL,
    "c_answer" text NOT NULL,
    "n_jgzs" integer NULL,
    "n_like" integer NOT NULL,
    "c_lastmodify" character varying(100) NOT NULL,
    "c_ip" character varying(300) NULL,
    "c_nyr" character varying(100) NULL,
    "c_ny" character varying(100) NULL,
    "dt_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_t_ai_feedback_like" to table: "t_ai_feedback"
CREATE INDEX "i_t_ai_feedback_like" ON "db_wenku"."t_ai_feedback" ("n_like");
-- Create index "i_t_ai_feedback_type" to table: "t_ai_feedback"
CREATE INDEX "i_t_ai_feedback_type" ON "db_wenku"."t_ai_feedback" ("c_type");
-- Set comment to table: "t_ai_feedback"
COMMENT ON TABLE "db_wenku"."t_ai_feedback" IS 'AI数据表';
-- Set comment to column: "c_id" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_corpid" IS '单位代码';
-- Set comment to column: "c_groupid" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_groupid" IS 'groupId，问答里面可能有记录groupId相同，起草和改写没有';
-- Set comment to column: "c_type" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_type" IS '类型，起草|改写|问答';
-- Set comment to column: "c_input" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_input" IS '用户输入，改写可能为空';
-- Set comment to column: "c_gxlx" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_gxlx" IS '改写类型，精简|段落扩写|续写|总结|大纲扩写，只有改写有值，起草和问答为空';
-- Set comment to column: "c_xznr" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_xznr" IS '改写时选中的内容，只有改写有值，起草和问答为空';
-- Set comment to column: "c_prompt" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_prompt" IS '问题';
-- Set comment to column: "c_answer" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_answer" IS '答案';
-- Set comment to column: "n_like" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."n_like" IS '是否点赞，0：未评价，1：点赞，-1：点差';
-- Set comment to column: "c_lastmodify" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_lastmodify" IS '最后修改时间';
-- Set comment to column: "c_ip" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_ip" IS '请求ip';
-- Set comment to column: "c_nyr" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_nyr" IS '年月日，格式为：yyyy年MM月dd日';
-- Set comment to column: "c_ny" on table: "t_ai_feedback"
COMMENT ON COLUMN "db_wenku"."t_ai_feedback"."c_ny" IS '年月，格式为：yyyy年MM月';
-- Create "t_analyze" table
CREATE TABLE "db_wenku"."t_analyze" (
    "c_id" character varying(32) NOT NULL,
    "c_type" character varying(300) NULL,
    "c_role" character varying(300) NULL,
    "c_content" text NULL,
    "n_retain" integer NULL DEFAULT 0,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_analyze"
COMMENT ON COLUMN "db_wenku"."t_analyze"."c_id" IS '编号';
-- Set comment to column: "c_type" on table: "t_analyze"
COMMENT ON COLUMN "db_wenku"."t_analyze"."c_type" IS '文章类型';
-- Set comment to column: "c_role" on table: "t_analyze"
COMMENT ON COLUMN "db_wenku"."t_analyze"."c_role" IS '角色类型';
-- Set comment to column: "c_content" on table: "t_analyze"
COMMENT ON COLUMN "db_wenku"."t_analyze"."c_content" IS '提示词内容';
-- Set comment to column: "n_retain" on table: "t_analyze"
COMMENT ON COLUMN "db_wenku"."t_analyze"."n_retain" IS '是否保留';
-- Create "t_app" table
CREATE TABLE "db_wenku"."t_app" (
    "c_id" character varying(32) NOT NULL,
    "c_app_id" character varying(100) NULL,
    "c_name" character varying(300) NULL,
    "c_api_key" character varying(300) NULL,
    "c_mode" character varying(32) NULL,
    "c_description" character varying(1000) NULL,
    "c_icon" text NULL,
    "c_workspace_name" character varying(300) NULL,
    "dt_create_time" timestamp NULL,
    "c_enable" character varying(50) NOT NULL DEFAULT '1',
    "n_sort" integer NOT NULL DEFAULT 1,
    "c_bg" character varying(100) NULL,
    "c_emoji" character varying(10) NULL,
    "c_av_flag" character varying(2) NULL,
    CONSTRAINT "pk_t_app" PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_app"
COMMENT ON TABLE "db_wenku"."t_app" IS '应用表';
-- Set comment to column: "c_app_id" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."c_app_id" IS '应用appId';
-- Set comment to column: "c_name" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."c_name" IS '应用名称';
-- Set comment to column: "c_api_key" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."c_api_key" IS '应用密钥';
-- Set comment to column: "c_mode" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."c_mode" IS '应用类型';
-- Set comment to column: "c_description" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."c_description" IS '应用描述';
-- Set comment to column: "c_workspace_name" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."c_workspace_name" IS '应用所属工作空间名称';
-- Set comment to column: "dt_create_time" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."dt_create_time" IS '创建时间';
-- Set comment to column: "c_enable" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."c_enable" IS '是否启用';
-- Set comment to column: "n_sort" on table: "t_app"
COMMENT ON COLUMN "db_wenku"."t_app"."n_sort" IS '应用顺序';
-- Create "t_app_chunk" table
CREATE TABLE "db_wenku"."t_app_chunk" (
    "c_id" character varying(50) NOT NULL,
    "c_app_id" character varying(50) NULL,
    "c_conversation_id" character varying(300) NULL,
    "c_user_id" character varying(50) NULL,
    "c_message_id" character varying(300) NULL,
    "c_chunks" text NULL,
    "dt_create_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_aty_cfgid" table
CREATE TABLE "db_wenku"."t_aty_cfgid" (
    "c_key" character varying(100) NOT NULL,
    "n_value" numeric(17,2) NOT NULL DEFAULT 0,
    "n_fldlength" numeric(3) NOT NULL DEFAULT 16,
    CONSTRAINT "pk_aty_cfgid" PRIMARY KEY ("c_key")
);
-- Create "t_aty_classcode" table
CREATE TABLE "db_wenku"."t_aty_classcode" (
    "c_type" character varying(100) NOT NULL,
    "c_code" character varying(100) NOT NULL,
    "c_pid" character varying(300) NULL,
    "c_name" character varying(300) NOT NULL,
    "n_valid" integer NOT NULL,
    "c_dmjp" character varying(300) NULL,
    "n_order" integer NOT NULL,
    "c_ext" text NULL,
    CONSTRAINT "pk_t_aty_classcode" PRIMARY KEY ("c_type", "c_code")
);
-- Create "t_aty_code" table
CREATE TABLE "db_wenku"."t_aty_code" (
    "c_pid" character varying(50) NOT NULL DEFAULT '0',
    "c_code" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "n_kwh" numeric(3) NOT NULL DEFAULT 2,
    "c_levelinfo" character varying(300) NULL,
    "n_valid" numeric(3) NOT NULL DEFAULT 1,
    "n_order" smallint NOT NULL DEFAULT 1,
    "c_dmjp" character varying(300) NULL,
    CONSTRAINT "pk_aty_code" PRIMARY KEY ("c_pid", "c_code")
);
-- Create "t_aty_codetype" table
CREATE TABLE "db_wenku"."t_aty_codetype" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "n_valid" numeric(3) NOT NULL DEFAULT 1,
    "n_sfkwh" numeric(3) NOT NULL DEFAULT 2,
    CONSTRAINT "pk_aty_codetype" PRIMARY KEY ("c_id")
);
-- Create "t_aty_config" table
CREATE TABLE "db_wenku"."t_aty_config" (
    "c_id" character varying(32) NOT NULL,
    "c_key" character varying(100) NOT NULL,
    "c_value" character varying(300) NULL,
    "c_descript" text NULL,
    CONSTRAINT "pk_t_aty_config" PRIMARY KEY ("c_id")
);
-- Create "t_aty_corp" table
CREATE TABLE "db_wenku"."t_aty_corp" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_pid" character varying(300) NULL,
    "n_level" numeric(3) NOT NULL DEFAULT 4,
    "c_gbm" character varying(300) NULL,
    "c_alias" character varying(300) NULL,
    "n_valid" numeric(3) NOT NULL DEFAULT 1,
    "n_order" smallint NOT NULL DEFAULT 1,
    "c_ext" text NULL,
    CONSTRAINT "pk_aty_corp" PRIMARY KEY ("c_id")
);
-- Create "t_aty_dept" table
CREATE TABLE "db_wenku"."t_aty_dept" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_pid" character varying(300) NULL,
    "c_corp" character varying(300) NULL,
    "c_alias" character varying(300) NULL,
    "n_valid" numeric(3) NOT NULL DEFAULT 1,
    "n_order" smallint NOT NULL DEFAULT 1,
    "c_ext" text NULL,
    CONSTRAINT "pk_aty_dept" PRIMARY KEY ("c_id")
);
-- Create "t_aty_dictcustom" table
CREATE TABLE "db_wenku"."t_aty_dictcustom" (
    "c_id" character varying(32) NOT NULL,
    "c_groupid" character varying(300) NOT NULL,
    "c_tablekey" character varying(300) NULL,
    "c_tablename" character varying(300) NULL,
    "c_fieldname" character varying(300) NULL,
    "c_propname" character varying(300) NULL,
    "c_value" character varying(300) NULL,
    CONSTRAINT "pk_aty_dictcustom" PRIMARY KEY ("c_id")
);
-- Create "t_aty_log" table
CREATE TABLE "db_wenku"."t_aty_log" (
    "c_id" character varying(32) NOT NULL,
    "c_type" character varying(50) NOT NULL,
    "c_module" character varying(300) NOT NULL,
    "c_function" character varying(300) NOT NULL,
    "c_content" text NOT NULL,
    "dt_time" timestamp NOT NULL,
    "c_result" character varying(100) NOT NULL,
    "c_host" character varying(100) NOT NULL,
    "c_userid" character varying(32) NULL,
    "c_username" character varying(300) NULL,
    "c_loginid" character varying(300) NULL,
    "c_corpid" character varying(32) NULL,
    "c_corpname" character varying(300) NULL,
    "c_deptid" character varying(32) NULL,
    "c_deptname" character varying(300) NULL,
    "c_md5" character varying(32) NOT NULL,
    "c_ext" text NULL,
    CONSTRAINT "pk_t_aty_log" PRIMARY KEY ("c_id")
);
-- Create "t_aty_log_archive" table
CREATE TABLE "db_wenku"."t_aty_log_archive" (
    "c_id" character varying(32) NOT NULL,
    "c_type" character varying(50) NOT NULL,
    "c_month" character varying(50) NOT NULL,
    "c_descript" text NULL,
    "n_count" integer NULL,
    "dt_time" date NOT NULL,
    CONSTRAINT "pk_t_aty_log_archive" PRIMARY KEY ("c_id")
);
-- Create "t_aty_log_back" table
CREATE TABLE "db_wenku"."t_aty_log_back" (
    "c_id" character varying(32) NOT NULL,
    "c_type" character varying(50) NOT NULL,
    "c_module" character varying(300) NOT NULL,
    "c_function" character varying(300) NOT NULL,
    "c_content" text NOT NULL,
    "dt_time" date NOT NULL,
    "c_result" character varying(100) NOT NULL,
    "c_host" character varying(100) NOT NULL,
    "c_userid" character varying(32) NULL,
    "c_username" character varying(300) NULL,
    "c_loginid" character varying(300) NULL,
    "c_corpid" character varying(32) NULL,
    "c_corpname" character varying(300) NULL,
    "c_deptid" character varying(32) NULL,
    "c_deptname" character varying(300) NULL,
    "c_md5" character varying(32) NOT NULL,
    "c_ext" text NULL,
    CONSTRAINT "pk_t_aty_log_back" PRIMARY KEY ("c_id")
);
-- Create "t_aty_planexeclog" table
CREATE TABLE "db_wenku"."t_aty_planexeclog" (
    "c_id" character varying(32) NOT NULL,
    "c_planid" character varying(32) NULL,
    "n_logtype" numeric(3) NULL,
    "d_time" timestamp NULL,
    "c_loginfo" character varying(300) NULL,
    "c_logexception" text NULL,
    CONSTRAINT "pk_aty_planexeclog" PRIMARY KEY ("c_id")
);
-- Create "t_aty_right" table
CREATE TABLE "db_wenku"."t_aty_right" (
    "c_rightkey" character varying(150) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_descript" character varying(300) NULL,
    "n_order" integer NULL,
    CONSTRAINT "pk_aty_right" PRIMARY KEY ("c_rightkey")
);
-- Create "t_aty_role" table
CREATE TABLE "db_wenku"."t_aty_role" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_descript" character varying(300) NULL,
    "n_xtgy" numeric(3) NULL DEFAULT 2,
    "n_valid" numeric(3) NULL DEFAULT 1,
    "n_order" smallint NULL DEFAULT 1,
    CONSTRAINT "pk_aty_role" PRIMARY KEY ("c_id")
);
-- Create "t_aty_role_right" table
CREATE TABLE "db_wenku"."t_aty_role_right" (
    "c_id" character varying(32) NOT NULL,
    "c_roleid" character varying(300) NOT NULL,
    "c_rightkey" character varying(300) NOT NULL,
    CONSTRAINT "pk_aty_role_right" PRIMARY KEY ("c_id")
);
-- Create "t_aty_user" table
CREATE TABLE "db_wenku"."t_aty_user" (
    "c_id" character varying(50) NOT NULL,
    "c_loginid" character varying(300) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_password" character varying(100) NOT NULL DEFAULT 'D41D8CD98F00B204E9800998ECF8427E',
    "c_mail" character varying(300) NULL,
    "c_ip" character varying(300) NULL,
    "c_xmjp" character varying(300) NULL,
    "c_corp" character varying(300) NULL,
    "c_dept" character varying(300) NULL,
    "n_valid" numeric(3) NOT NULL DEFAULT 1,
    "n_order" smallint NOT NULL DEFAULT 1,
    "n_retry" integer NULL,
    "dt_password_update" date NULL,
    "c_ext" text NULL,
    CONSTRAINT "pk_aty_user" PRIMARY KEY ("c_id")
);
-- Create "t_aty_user_right" table
CREATE TABLE "db_wenku"."t_aty_user_right" (
    "c_id" character varying(32) NOT NULL,
    "c_userid" character varying(300) NOT NULL,
    "n_type" numeric(3) NOT NULL DEFAULT 1,
    "c_roleid" character varying(300) NULL,
    "c_rightkey" character varying(300) NULL,
    CONSTRAINT "pk_aty_user_right" PRIMARY KEY ("c_id")
);
-- Create "t_aty_writ" table
CREATE TABLE "db_wenku"."t_aty_writ" (
    "c_id" character varying(32) NOT NULL,
    "c_tid" character varying(32) NOT NULL,
    "c_name" character varying(300) NULL,
    "n_version" numeric(3) NULL,
    "i_writ" bytea NULL,
    "c_writhtml" text NULL,
    "d_updatetime" timestamp NULL,
    "c_ryid" character varying(300) NULL,
    CONSTRAINT "pk_aty_writ" PRIMARY KEY ("c_id")
);
-- Create "t_aty_writlog" table
CREATE TABLE "db_wenku"."t_aty_writlog" (
    "c_id" character varying(32) NOT NULL,
    "c_writid" character varying(32) NOT NULL,
    "c_name" character varying(300) NULL,
    "n_version" numeric(3) NULL,
    "i_writ" bytea NULL,
    "c_writhtml" bytea NULL,
    "d_updatetime" timestamp NULL,
    "c_ryid" character varying(300) NULL,
    CONSTRAINT "pk_aty_writlog" PRIMARY KEY ("c_id")
);
-- Create "t_audio" table
CREATE TABLE "db_wenku"."t_audio" (
    "c_id" character varying(50) NOT NULL,
    "c_user_id" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "n_state" integer NULL,
    "c_duration" character varying(300) NULL,
    "dt_create_time" timestamp NULL,
    "c_ext" text NULL,
    "c_notes" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_audio_name" to table: "t_audio"
CREATE INDEX "i_audio_name" ON "db_wenku"."t_audio" ("c_name");
-- Create index "i_audio_user_id" to table: "t_audio"
CREATE INDEX "i_audio_user_id" ON "db_wenku"."t_audio" ("c_user_id");
-- Create "t_audio_duration_stats" table
CREATE TABLE "db_wenku"."t_audio_duration_stats" (
    "c_user_id" character varying(300) NOT NULL,
    "n_total_duration_seconds" bigint NULL DEFAULT 0,
    PRIMARY KEY ("c_user_id")
);
-- Create index "i_audio_duration_stats_user_id" to table: "t_audio_duration_stats"
CREATE INDEX "i_audio_duration_stats_user_id" ON "db_wenku"."t_audio_duration_stats" ("c_user_id");
-- Set comment to table: "t_audio_duration_stats"
COMMENT ON TABLE "db_wenku"."t_audio_duration_stats" IS '用户录音转写总时长统计表';
-- Set comment to column: "c_user_id" on table: "t_audio_duration_stats"
COMMENT ON COLUMN "db_wenku"."t_audio_duration_stats"."c_user_id" IS '用户ID';
-- Set comment to column: "n_total_duration_seconds" on table: "t_audio_duration_stats"
COMMENT ON COLUMN "db_wenku"."t_audio_duration_stats"."n_total_duration_seconds" IS '总录音时长（秒）';
-- Create "t_audio_sep" table
CREATE TABLE "db_wenku"."t_audio_sep" (
    "c_id" character varying(50) NOT NULL,
    "c_audio_id" character varying(50) NULL,
    "c_name" character varying(300) NULL,
    "c_begin_time" character varying(300) NULL,
    "c_end_time" character varying(300) NULL,
    "c_content" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_audio_sep_audio_id" to table: "t_audio_sep"
CREATE INDEX "i_audio_sep_audio_id" ON "db_wenku"."t_audio_sep" ("c_audio_id");
-- Create "t_audio_summary" table
CREATE TABLE "db_wenku"."t_audio_summary" (
    "c_id" character varying(50) NOT NULL,
    "c_audio_id" character varying(50) NULL,
    "c_title" character varying(300) NULL,
    "c_content" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_audio_summary_audio_id" to table: "t_audio_summary"
CREATE INDEX "i_audio_summary_audio_id" ON "db_wenku"."t_audio_summary" ("c_audio_id");
-- Create "t_audioconfig" table
CREATE TABLE "db_wenku"."t_audioconfig" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NULL,
    "c_key" character varying(100) NULL,
    "c_value" character varying(300) NULL,
    "n_order" integer NULL,
    "dt_create_time" timestamp NULL,
    "dt_update_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_audiot_hotword" table
CREATE TABLE "db_wenku"."t_audiot_hotword" (
    "c_id" character varying(50) NOT NULL,
    "c_user_id" character varying(300) NULL,
    "c_hotword_id" character varying(300) NULL,
    "c_hotword" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_audio_hotword_hotword_id" to table: "t_audiot_hotword"
CREATE INDEX "i_audio_hotword_hotword_id" ON "db_wenku"."t_audiot_hotword" ("c_hotword_id");
-- Create index "i_audio_hotword_user_id" to table: "t_audiot_hotword"
CREATE INDEX "i_audio_hotword_user_id" ON "db_wenku"."t_audiot_hotword" ("c_user_id");
-- Create "t_bim_org" table
CREATE TABLE "db_wenku"."t_bim_org" (
    "c_id" character varying(50) NOT NULL,
    "c_bim_org_id" character varying(300) NULL,
    "c_local_org_id" character varying(300) NULL,
    "c_org_name" character varying(300) NULL,
    "c_par_bim_org_id" character varying(300) NULL,
    "n_enable" smallint NULL DEFAULT 1,
    "dt_cjsj" timestamp NULL,
    "dt_gxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_t_bim_org_bim_id" to table: "t_bim_org"
CREATE INDEX "i_t_bim_org_bim_id" ON "db_wenku"."t_bim_org" ("c_bim_org_id");
-- Create index "i_t_bim_org_local_id" to table: "t_bim_org"
CREATE INDEX "i_t_bim_org_local_id" ON "db_wenku"."t_bim_org" ("c_local_org_id");
-- Set comment to table: "t_bim_org"
COMMENT ON TABLE "db_wenku"."t_bim_org" IS 'BIM 组织机构映射表';
-- Set comment to column: "c_id" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."c_id" IS '主键';
-- Set comment to column: "c_bim_org_id" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."c_bim_org_id" IS '平台组织机构主键';
-- Set comment to column: "c_local_org_id" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."c_local_org_id" IS 'Artery 内部组织机构 ID';
-- Set comment to column: "c_org_name" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."c_org_name" IS '组织机构名称';
-- Set comment to column: "c_par_bim_org_id" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."c_par_bim_org_id" IS '平台上级组织机构主键';
-- Set comment to column: "n_enable" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."n_enable" IS '是否启用：1-启用，0-禁用';
-- Set comment to column: "dt_cjsj" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."dt_cjsj" IS '创建时间';
-- Set comment to column: "dt_gxsj" on table: "t_bim_org"
COMMENT ON COLUMN "db_wenku"."t_bim_org"."dt_gxsj" IS '更新时间';
-- Create "t_bim_user" table
CREATE TABLE "db_wenku"."t_bim_user" (
    "c_id" character varying(50) NOT NULL,
    "c_bim_uid" character varying(300) NULL,
    "c_local_user_id" character varying(300) NULL,
    "c_login_name" character varying(300) NULL,
    "c_full_name" character varying(300) NULL,
    "c_bim_org_id" character varying(300) NULL,
    "c_personal_confidentiality_level" character varying(64) NULL,
    "n_enable" smallint NULL DEFAULT 1,
    "dt_cjsj" timestamp NULL,
    "dt_gxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_t_bim_user_bim_uid" to table: "t_bim_user"
CREATE INDEX "i_t_bim_user_bim_uid" ON "db_wenku"."t_bim_user" ("c_bim_uid");
-- Create index "i_t_bim_user_local_id" to table: "t_bim_user"
CREATE INDEX "i_t_bim_user_local_id" ON "db_wenku"."t_bim_user" ("c_local_user_id");
-- Create index "i_t_bim_user_login_name" to table: "t_bim_user"
CREATE INDEX "i_t_bim_user_login_name" ON "db_wenku"."t_bim_user" ("c_login_name");
-- Create index "i_t_bim_user_pcl" to table: "t_bim_user"
CREATE INDEX "i_t_bim_user_pcl" ON "db_wenku"."t_bim_user" ("c_personal_confidentiality_level");
-- Set comment to table: "t_bim_user"
COMMENT ON TABLE "db_wenku"."t_bim_user" IS 'BIM 用户映射表';
-- Set comment to column: "c_id" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."c_id" IS '主键';
-- Set comment to column: "c_bim_uid" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."c_bim_uid" IS '平台账号主键';
-- Set comment to column: "c_local_user_id" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."c_local_user_id" IS 'Artery 内部用户 ID';
-- Set comment to column: "c_login_name" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."c_login_name" IS '登录名';
-- Set comment to column: "c_full_name" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."c_full_name" IS '用户姓名';
-- Set comment to column: "c_bim_org_id" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."c_bim_org_id" IS '平台组织机构主键';
-- Set comment to column: "c_personal_confidentiality_level" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."c_personal_confidentiality_level" IS '人员涉密等级';
-- Set comment to column: "n_enable" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."n_enable" IS '是否启用：1-启用，0-禁用';
-- Set comment to column: "dt_cjsj" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."dt_cjsj" IS '创建时间';
-- Set comment to column: "dt_gxsj" on table: "t_bim_user"
COMMENT ON COLUMN "db_wenku"."t_bim_user"."dt_gxsj" IS '更新时间';
-- Create "t_chat_history" table
CREATE TABLE "db_wenku"."t_chat_history" (
    "c_id" bigserial NOT NULL,
    "c_session_id" character varying(50) NOT NULL,
    "c_user_id" character varying(50) NOT NULL,
    "c_message_type" character varying(20) NOT NULL,
    "c_content" text NOT NULL,
    "c_created_at" bigint NOT NULL,
    "c_updated_at" timestamp NULL,
    "c_message_order" integer NULL,
    "c_title" text NULL,
    "c_model_used" character varying(100) NULL,
    "c_tags" character varying(255) NULL,
    "c_is_deleted" boolean NULL DEFAULT false,
    "c_is_pinned" boolean NULL DEFAULT false,
    "c_conversation_id" character varying(50) NULL,
    "c_main_content" text NULL,
    "c_answer_content" text NULL,
    "c_message_id" character varying(300) NULL,
    CONSTRAINT "chat_history_pkey" PRIMARY KEY ("c_id"),
    CONSTRAINT "chat_history_t_message_type_check" CHECK ((c_message_type)::text = ANY ((ARRAY['user'::character varying, 'ai'::character varying])::text[]))
);
-- Create index "idx_chat_history_conversation_id" to table: "t_chat_history"
CREATE INDEX "idx_chat_history_conversation_id" ON "db_wenku"."t_chat_history" ("c_conversation_id");
-- Create "t_client" table
CREATE TABLE "db_wenku"."t_client" (
    "c_id" character varying(50) NOT NULL,
    "c_ip" character varying(300) NULL,
    "dt_cjsj" timestamp NULL,
    "dt_zhgxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_client"
COMMENT ON TABLE "db_wenku"."t_client" IS '客户端记录表';
-- Set comment to column: "c_id" on table: "t_client"
COMMENT ON COLUMN "db_wenku"."t_client"."c_id" IS 'id';
-- Set comment to column: "c_ip" on table: "t_client"
COMMENT ON COLUMN "db_wenku"."t_client"."c_ip" IS 'IP';
-- Set comment to column: "dt_cjsj" on table: "t_client"
COMMENT ON COLUMN "db_wenku"."t_client"."dt_cjsj" IS '创建时间';
-- Set comment to column: "dt_zhgxsj" on table: "t_client"
COMMENT ON COLUMN "db_wenku"."t_client"."dt_zhgxsj" IS '最后更新时间';
-- Create "t_client_user" table
CREATE TABLE "db_wenku"."t_client_user" (
    "c_id" character varying(50) NOT NULL,
    "c_user" character varying(100) NULL,
    "dt_cjsj" timestamp NULL,
    "dt_zhgxsj" timestamp NULL,
    CONSTRAINT "t_client_user_pk" PRIMARY KEY ("c_id")
);
-- Create "t_clientui" table
CREATE TABLE "db_wenku"."t_clientui" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "c_group" character varying(100) NULL,
    "c_normalshowtype" character varying(10) NULL,
    "c_embedshowtype" character varying(10) NULL,
    "c_normalmessage" character varying(300) NULL,
    "c_embedmessage" character varying(300) NULL,
    "dt_createtime" timestamp NULL,
    "n_order" integer NULL,
    "dt_updatetime" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_clientui"
COMMENT ON TABLE "db_wenku"."t_clientui" IS '客户端显示配置表';
-- Set comment to column: "c_id" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_corpid" IS '单位代码';
-- Set comment to column: "c_name" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_name" IS '显示项名称';
-- Set comment to column: "c_group" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_group" IS '显示项关键字';
-- Set comment to column: "c_normalshowtype" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_normalshowtype" IS 'office中显示类型';
-- Set comment to column: "c_embedshowtype" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_embedshowtype" IS '页面中显示类型';
-- Set comment to column: "c_normalmessage" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_normalmessage" IS 'office中提示信息';
-- Set comment to column: "c_embedmessage" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."c_embedmessage" IS '页面中提示信息';
-- Set comment to column: "dt_createtime" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."dt_createtime" IS '创建时间';
-- Set comment to column: "n_order" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."n_order" IS '显示顺序';
-- Set comment to column: "dt_updatetime" on table: "t_clientui"
COMMENT ON COLUMN "db_wenku"."t_clientui"."dt_updatetime" IS '更新时间';
-- Create "t_config" table
CREATE TABLE "db_wenku"."t_config" (
    "c_id" character varying(32) NOT NULL,
    "c_key" character varying(100) NOT NULL,
    "c_value" character varying(300) NULL,
    "c_descript" text NULL,
    "dt_gxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_config"
COMMENT ON COLUMN "db_wenku"."t_config"."c_id" IS '编号';
-- Set comment to column: "c_key" on table: "t_config"
COMMENT ON COLUMN "db_wenku"."t_config"."c_key" IS '配置项的关键字';
-- Set comment to column: "c_value" on table: "t_config"
COMMENT ON COLUMN "db_wenku"."t_config"."c_value" IS '配置项的值';
-- Set comment to column: "c_descript" on table: "t_config"
COMMENT ON COLUMN "db_wenku"."t_config"."c_descript" IS '配置项的描述';
-- Set comment to column: "dt_gxsj" on table: "t_config"
COMMENT ON COLUMN "db_wenku"."t_config"."dt_gxsj" IS '更新时间';
-- Create "t_corpauth" table
CREATE TABLE "db_wenku"."t_corpauth" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_corpname" character varying(300) NULL,
    "dt_expiredtime" timestamp NULL,
    "n_maxnumberofuser" integer NULL,
    "c_admin" character varying(300) NULL,
    "dt_time" timestamp NULL,
    "c_sfcjzh" character varying(50) NULL,
    "n_cjzhgs" integer NULL,
    "c_zhqz" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_corpauth"
COMMENT ON TABLE "db_wenku"."t_corpauth" IS '单位授权信息表';
-- Set comment to column: "c_id" on table: "t_corpauth"
COMMENT ON COLUMN "db_wenku"."t_corpauth"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_corpauth"
COMMENT ON COLUMN "db_wenku"."t_corpauth"."c_corpid" IS '单位代码';
-- Set comment to column: "dt_expiredtime" on table: "t_corpauth"
COMMENT ON COLUMN "db_wenku"."t_corpauth"."dt_expiredtime" IS '到期时间';
-- Set comment to column: "n_maxnumberofuser" on table: "t_corpauth"
COMMENT ON COLUMN "db_wenku"."t_corpauth"."n_maxnumberofuser" IS '最大用户数';
-- Set comment to column: "c_admin" on table: "t_corpauth"
COMMENT ON COLUMN "db_wenku"."t_corpauth"."c_admin" IS '管理员账号';
-- Set comment to column: "dt_time" on table: "t_corpauth"
COMMENT ON COLUMN "db_wenku"."t_corpauth"."dt_time" IS '更新时间';
-- Create "t_corpconfig" table
CREATE TABLE "db_wenku"."t_corpconfig" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_key" character varying(100) NULL,
    "c_value" character varying(300) NULL,
    "dt_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_corpconfig"
COMMENT ON TABLE "db_wenku"."t_corpconfig" IS '单位配置表';
-- Set comment to column: "c_id" on table: "t_corpconfig"
COMMENT ON COLUMN "db_wenku"."t_corpconfig"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_corpconfig"
COMMENT ON COLUMN "db_wenku"."t_corpconfig"."c_corpid" IS '单位代码';
-- Set comment to column: "c_key" on table: "t_corpconfig"
COMMENT ON COLUMN "db_wenku"."t_corpconfig"."c_key" IS '配置项关键字';
-- Set comment to column: "c_value" on table: "t_corpconfig"
COMMENT ON COLUMN "db_wenku"."t_corpconfig"."c_value" IS '显示项关键字';
-- Set comment to column: "dt_time" on table: "t_corpconfig"
COMMENT ON COLUMN "db_wenku"."t_corpconfig"."dt_time" IS '更新时间';
-- Create "t_corpuser" table
CREATE TABLE "db_wenku"."t_corpuser" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "dt_time" timestamp NULL,
    "c_ny" character varying(100) NULL,
    "c_nyr" character varying(100) NULL,
    "c_corpname" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_corpuser"
COMMENT ON TABLE "db_wenku"."t_corpuser" IS '单位用户登录记录表';
-- Set comment to column: "c_id" on table: "t_corpuser"
COMMENT ON COLUMN "db_wenku"."t_corpuser"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_corpuser"
COMMENT ON COLUMN "db_wenku"."t_corpuser"."c_corpid" IS '单位代码';
-- Set comment to column: "c_userid" on table: "t_corpuser"
COMMENT ON COLUMN "db_wenku"."t_corpuser"."c_userid" IS '用户登录标识';
-- Set comment to column: "dt_time" on table: "t_corpuser"
COMMENT ON COLUMN "db_wenku"."t_corpuser"."dt_time" IS '更新时间';
-- Create "t_credential_keys" table
CREATE TABLE "db_wenku"."t_credential_keys" (
    "c_id" character varying(50) NOT NULL,
    "c_key" character varying(200) NULL,
    "c_login_id" character varying(50) NULL,
    "c_show_name" character varying(50) NULL,
    "n_timestamp" bigint NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_custom_index_data" table
CREATE TABLE "db_wenku"."t_custom_index_data" (
    "c_id" character varying(50) NOT NULL,
    "c_es_id" character varying(300) NULL,
    "c_syn_id" character varying(300) NULL,
    "c_index" character varying(50) NULL,
    "c_dataset_id" character varying(300) NULL,
    "n_type" integer NULL,
    "dt_syn_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "t_custom_index_data_c_es_id_index" to table: "t_custom_index_data"
CREATE INDEX "t_custom_index_data_c_es_id_index" ON "db_wenku"."t_custom_index_data" ("c_es_id");
-- Create index "t_custom_index_data_c_index_index" to table: "t_custom_index_data"
CREATE INDEX "t_custom_index_data_c_index_index" ON "db_wenku"."t_custom_index_data" ("c_index");
-- Create "t_data_import_module" table
CREATE TABLE "db_wenku"."t_data_import_module" (
    "c_id" character varying(50) NOT NULL,
    "c_module" character varying(50) NULL,
    "c_name" character varying(255) NULL,
    "c_user" character varying NULL,
    "c_right" text NULL,
    "dt_time" timestamp NULL,
    "c_type" character varying(30) NULL,
    "login_type" integer NULL DEFAULT 0,
    "c_topic" character varying(32) NULL,
    "c_section" character varying(32) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_topic" on table: "t_data_import_module"
COMMENT ON COLUMN "db_wenku"."t_data_import_module"."c_topic" IS '主题';
-- Set comment to column: "c_section" on table: "t_data_import_module"
COMMENT ON COLUMN "db_wenku"."t_data_import_module"."c_section" IS '板块';
-- Create "t_data_module" table
CREATE TABLE "db_wenku"."t_data_module" (
    "c_id" character varying(32) NOT NULL,
    "c_name" character varying(100) NULL,
    "c_index" character varying(100) NULL,
    "c_type" character varying(50) NULL,
    "n_order" integer NOT NULL,
    "c_knowledge_id" character varying(300) NULL,
    "c_dept" text NULL,
    "c_dept_type" text NULL,
    "c_doc_type" character varying(30) NULL DEFAULT 'normal',
    PRIMARY KEY ("c_id")
);
-- Create "t_dify_zsk" table
CREATE TABLE "db_wenku"."t_dify_zsk" (
    "c_id" character varying(32) NOT NULL,
    "c_zsk_id" character varying(300) NULL,
    "c_zsk_name" character varying(300) NULL,
    "c_zsk_mate" text NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_dify_zsk"
COMMENT ON COLUMN "db_wenku"."t_dify_zsk"."c_id" IS '编号';
-- Set comment to column: "c_zsk_id" on table: "t_dify_zsk"
COMMENT ON COLUMN "db_wenku"."t_dify_zsk"."c_zsk_id" IS '知识库的ID';
-- Set comment to column: "c_zsk_name" on table: "t_dify_zsk"
COMMENT ON COLUMN "db_wenku"."t_dify_zsk"."c_zsk_name" IS '知识库的名称';
-- Set comment to column: "c_zsk_mate" on table: "t_dify_zsk"
COMMENT ON COLUMN "db_wenku"."t_dify_zsk"."c_zsk_mate" IS '知识库的元数据';
-- Create "t_ding_user" table
CREATE TABLE "db_wenku"."t_ding_user" (
    "c_id" character varying(32) NOT NULL,
    "c_token" character varying(50) NULL,
    "c_user_info" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_ding_user_token" to table: "t_ding_user"
CREATE INDEX "i_ding_user_token" ON "db_wenku"."t_ding_user" ("c_token");
-- Create "t_docsearch" table
CREATE TABLE "db_wenku"."t_docsearch" (
    "c_id" character varying(50) NOT NULL,
    "c_ostype" character varying(50) NOT NULL,
    "c_cputype" character varying(50) NOT NULL,
    "c_filesize" character varying(300) NOT NULL,
    "c_objname" character varying(900) NOT NULL,
    "c_exeaddr" text NOT NULL,
    "d_createtime" timestamp NOT NULL,
    "c_md5" character varying(300) NULL,
    "c_version" character varying(30) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."c_id" IS 'id';
-- Set comment to column: "c_ostype" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."c_ostype" IS '操作系统';
-- Set comment to column: "c_cputype" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."c_cputype" IS 'CPU类型';
-- Set comment to column: "c_filesize" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."c_filesize" IS '文件大小';
-- Set comment to column: "c_objname" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."c_objname" IS '文件名称';
-- Set comment to column: "c_exeaddr" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."c_exeaddr" IS '地址';
-- Set comment to column: "d_createtime" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."d_createtime" IS '创建时间';
-- Set comment to column: "c_md5" on table: "t_docsearch"
COMMENT ON COLUMN "db_wenku"."t_docsearch"."c_md5" IS 'MD5值';
-- Create "t_dyn_wj" table
CREATE TABLE "db_wenku"."t_dyn_wj" (
    "c_id" character varying(32) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_loginid" character varying(300) NULL,
    "c_username" character varying(300) NULL,
    "c_index" character varying(50) NULL,
    "c_section" character varying(50) NULL,
    "c_topic" character varying(300) NULL,
    "n_xysh" integer NULL,
    "n_jzjkj" integer NULL,
    "c_wjm" character varying(600) NULL,
    "c_wjlj" character varying(900) NULL,
    "c_wjms" character varying(900) NULL,
    "n_shzt" integer NULL,
    "dt_shsj" timestamp NULL,
    "c_shr" character varying(300) NULL,
    "c_shbtgyy" character varying(900) NULL,
    "n_drzt" integer NULL,
    "dt_drsj" timestamp NULL,
    "c_drjg" character varying(300) NULL,
    "dt_cjsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_dyn_wj"
COMMENT ON TABLE "db_wenku"."t_dyn_wj" IS '我的知识-文件表';
-- Set comment to column: "c_id" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_corpid" IS '单位id';
-- Set comment to column: "c_loginid" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_loginid" IS '登录标识';
-- Set comment to column: "c_username" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_username" IS '用户名';
-- Set comment to column: "c_index" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_index" IS '索引';
-- Set comment to column: "c_section" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_section" IS '板块';
-- Set comment to column: "c_topic" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_topic" IS '主题';
-- Set comment to column: "n_xysh" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."n_xysh" IS '是否需要审核';
-- Set comment to column: "n_jzjkj" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."n_jzjkj" IS '是否仅自己可见';
-- Set comment to column: "c_wjm" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_wjm" IS '文件名';
-- Set comment to column: "c_wjlj" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_wjlj" IS '文件路径';
-- Set comment to column: "c_wjms" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_wjms" IS '文件描述';
-- Set comment to column: "n_shzt" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."n_shzt" IS '审核状态';
-- Set comment to column: "dt_shsj" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."dt_shsj" IS '审核时间';
-- Set comment to column: "c_shr" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_shr" IS '审核人';
-- Set comment to column: "c_shbtgyy" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_shbtgyy" IS '审核不通过原因';
-- Set comment to column: "n_drzt" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."n_drzt" IS '导入状态';
-- Set comment to column: "dt_drsj" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."dt_drsj" IS '导入时间';
-- Set comment to column: "c_drjg" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."c_drjg" IS '导入结果';
-- Set comment to column: "dt_cjsj" on table: "t_dyn_wj"
COMMENT ON COLUMN "db_wenku"."t_dyn_wj"."dt_cjsj" IS '创建时间';
-- Create "t_favorite" table
CREATE TABLE "db_wenku"."t_favorite" (
    "c_id" character varying(50) NOT NULL,
    "c_docid" character varying(300) NOT NULL,
    "c_userid" character varying(300) NOT NULL,
    "c_time" character varying(100) NOT NULL,
    "c_doctype" character varying(50) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_favorite"
COMMENT ON COLUMN "db_wenku"."t_favorite"."c_id" IS 'id';
-- Set comment to column: "c_docid" on table: "t_favorite"
COMMENT ON COLUMN "db_wenku"."t_favorite"."c_docid" IS '素材id';
-- Set comment to column: "c_userid" on table: "t_favorite"
COMMENT ON COLUMN "db_wenku"."t_favorite"."c_userid" IS '用户id';
-- Set comment to column: "c_time" on table: "t_favorite"
COMMENT ON COLUMN "db_wenku"."t_favorite"."c_time" IS '创建时间';
-- Create "t_gwbj_client_version" table
CREATE TABLE "db_wenku"."t_gwbj_client_version" (
    "c_id" character varying(50) NOT NULL,
    "c_version" character varying(300) NOT NULL,
    "c_ostype" character varying(50) NOT NULL,
    "c_cputype" character varying(50) NOT NULL,
    "c_frame" character varying(50) NULL,
    "d_date" date NOT NULL,
    "lc_updatelog" text NULL,
    "c_filesize" character varying(300) NOT NULL,
    "c_objname" character varying(900) NOT NULL,
    "c_exeaddr" text NOT NULL,
    "d_createtime" timestamp NOT NULL,
    "n_iscurrent" integer NOT NULL,
    "c_md5" character varying(300) NULL,
    "c_clientid" character varying(300) NULL,
    "c_upgradeobjname" character varying(900) NULL,
    "c_upgradeaddr" character varying(900) NULL,
    "c_upgrademd5" character varying(300) NULL,
    "n_isforceupdate" integer NULL,
    "c_cdnaddr" character varying(900) NULL,
    "c_upgradecdnaddr" character varying(900) NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_gwbj_corp_config" table
CREATE TABLE "db_wenku"."t_gwbj_corp_config" (
    "c_id" character varying(32) NOT NULL,
    "c_tenant_id" character varying(300) NULL,
    "c_corpid" character varying(300) NULL,
    "c_isuse" integer NULL,
    "c_leader" text NULL,
    "c_department" text NULL,
    "c_extra" text NULL,
    "c_prompt" text NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_corp_config"
COMMENT ON TABLE "db_wenku"."t_gwbj_corp_config" IS '单位信息配置表';
-- Set comment to column: "c_id" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_id" IS '编号';
-- Set comment to column: "c_tenant_id" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_tenant_id" IS '租户编号';
-- Set comment to column: "c_corpid" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_corpid" IS '单位代码';
-- Set comment to column: "c_isuse" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_isuse" IS '是否使用单位配置';
-- Set comment to column: "c_leader" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_leader" IS '领导信息';
-- Set comment to column: "c_department" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_department" IS '部门信息';
-- Set comment to column: "c_extra" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_extra" IS '额外信息';
-- Set comment to column: "c_prompt" on table: "t_gwbj_corp_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_corp_config"."c_prompt" IS '提示词模板';
-- Create "t_gwbj_sc_zl" table
CREATE TABLE "db_wenku"."t_gwbj_sc_zl" (
    "c_id" character varying(50) NOT NULL,
    "c_userid" character varying(300) NOT NULL,
    "c_source" character varying(300) NOT NULL,
    "c_time" character varying(100) NOT NULL,
    "c_content" text NOT NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_sc_zl"
COMMENT ON TABLE "db_wenku"."t_gwbj_sc_zl" IS '公文编校收藏摘录';
-- Set comment to column: "c_id" on table: "t_gwbj_sc_zl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_sc_zl"."c_id" IS 'id';
-- Set comment to column: "c_userid" on table: "t_gwbj_sc_zl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_sc_zl"."c_userid" IS '用户id';
-- Set comment to column: "c_source" on table: "t_gwbj_sc_zl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_sc_zl"."c_source" IS '摘录来源';
-- Set comment to column: "c_time" on table: "t_gwbj_sc_zl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_sc_zl"."c_time" IS '创建时间';
-- Set comment to column: "c_content" on table: "t_gwbj_sc_zl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_sc_zl"."c_content" IS '摘录内容';
-- Create "t_gwbj_template" table
CREATE TABLE "db_wenku"."t_gwbj_template" (
    "c_id" character varying(32) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "c_filename" character varying(300) NULL,
    "c_type" character varying(300) NULL,
    "n_status" integer NULL,
    "dt_time" timestamp NULL,
    "c_groupid" character varying(50) NULL,
    "n_order" integer NULL,
    "c_filepath" character varying(900) NULL,
    "c_xmlpath" character varying(900) NULL,
    "c_imagepath" character varying(900) NULL,
    "c_content_id" character varying(300) NULL,
    "c_ban_id" character varying(300) NULL,
    "c_jsonpath" character varying(900) NULL,
    "c_attach_pos" integer NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_template"
COMMENT ON TABLE "db_wenku"."t_gwbj_template" IS '模版表';
-- Set comment to column: "c_id" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_id" IS '主键';
-- Set comment to column: "c_name" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_name" IS '模板名称';
-- Set comment to column: "c_filename" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_filename" IS '模板文件名';
-- Set comment to column: "c_type" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_type" IS '模版类型：default、import';
-- Set comment to column: "n_status" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."n_status" IS '状态';
-- Set comment to column: "dt_time" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."dt_time" IS '更新时间';
-- Set comment to column: "c_groupid" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_groupid" IS '分组id';
-- Set comment to column: "n_order" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."n_order" IS '显示顺序';
-- Set comment to column: "c_filepath" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_filepath" IS '模版文件路径';
-- Set comment to column: "c_xmlpath" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_xmlpath" IS 'xml文件路径';
-- Set comment to column: "c_imagepath" on table: "t_gwbj_template"
COMMENT ON COLUMN "db_wenku"."t_gwbj_template"."c_imagepath" IS '缩略图文件路径';
-- Create "t_gwbj_templategroup" table
CREATE TABLE "db_wenku"."t_gwbj_templategroup" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "n_status" integer NULL,
    "dt_time" timestamp NULL,
    "n_order" integer NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_templategroup"
COMMENT ON TABLE "db_wenku"."t_gwbj_templategroup" IS '模版分组表';
-- Set comment to column: "c_id" on table: "t_gwbj_templategroup"
COMMENT ON COLUMN "db_wenku"."t_gwbj_templategroup"."c_id" IS 'id';
-- Set comment to column: "c_name" on table: "t_gwbj_templategroup"
COMMENT ON COLUMN "db_wenku"."t_gwbj_templategroup"."c_name" IS '分组名称';
-- Set comment to column: "n_status" on table: "t_gwbj_templategroup"
COMMENT ON COLUMN "db_wenku"."t_gwbj_templategroup"."n_status" IS '状态';
-- Set comment to column: "dt_time" on table: "t_gwbj_templategroup"
COMMENT ON COLUMN "db_wenku"."t_gwbj_templategroup"."dt_time" IS '更新时间';
-- Create "t_gwbj_templatetime" table
CREATE TABLE "db_wenku"."t_gwbj_templatetime" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "dt_grouptime" timestamp NULL,
    "dt_templatetime" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_templatetime"
COMMENT ON TABLE "db_wenku"."t_gwbj_templatetime" IS '模版更新时间表';
-- Set comment to column: "c_id" on table: "t_gwbj_templatetime"
COMMENT ON COLUMN "db_wenku"."t_gwbj_templatetime"."c_id" IS 'id';
-- Set comment to column: "dt_grouptime" on table: "t_gwbj_templatetime"
COMMENT ON COLUMN "db_wenku"."t_gwbj_templatetime"."dt_grouptime" IS '分组最新更新时间';
-- Set comment to column: "dt_templatetime" on table: "t_gwbj_templatetime"
COMMENT ON COLUMN "db_wenku"."t_gwbj_templatetime"."dt_templatetime" IS '模版最新更新时间';
-- Create "t_gwbj_tj_errorword" table
CREATE TABLE "db_wenku"."t_gwbj_tj_errorword" (
    "c_id" character varying(32) NOT NULL,
    "c_word" character varying(900) NULL,
    "c_level" character varying(100) NULL,
    "c_errorcode" character varying(100) NULL,
    "c_message" character varying(100) NULL,
    "c_sentence" character varying(900) NULL,
    "dt_gxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_tj_errorword"
COMMENT ON TABLE "db_wenku"."t_gwbj_tj_errorword" IS '错词统计表';
-- Set comment to column: "c_id" on table: "t_gwbj_tj_errorword"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_errorword"."c_id" IS '编号';
-- Set comment to column: "c_word" on table: "t_gwbj_tj_errorword"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_errorword"."c_word" IS '错词';
-- Set comment to column: "c_level" on table: "t_gwbj_tj_errorword"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_errorword"."c_level" IS '错误级别';
-- Set comment to column: "c_errorcode" on table: "t_gwbj_tj_errorword"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_errorword"."c_errorcode" IS '错误类型';
-- Set comment to column: "c_message" on table: "t_gwbj_tj_errorword"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_errorword"."c_message" IS '错误原因';
-- Set comment to column: "c_sentence" on table: "t_gwbj_tj_errorword"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_errorword"."c_sentence" IS '所在句子';
-- Set comment to column: "dt_gxsj" on table: "t_gwbj_tj_errorword"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_errorword"."dt_gxsj" IS '更新时间';
-- Create "t_gwbj_tj_gwjd" table
CREATE TABLE "db_wenku"."t_gwbj_tj_gwjd" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_ip" character varying(300) NULL,
    "n_cwsl" integer NULL,
    "n_jgsl" integer NULL,
    "n_tssl" integer NULL,
    "n_jdzs" integer NULL,
    "dt_time" timestamp NULL,
    "c_nyr" character varying(300) NULL,
    "c_ny" character varying(300) NULL,
    "c_version" character varying(300) NULL,
    "c_arch" character varying(300) NULL,
    "c_ostype" character varying(300) NULL,
    "c_osversion" character varying(300) NULL,
    "c_mac" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_tj_gwjd"
COMMENT ON TABLE "db_wenku"."t_gwbj_tj_gwjd" IS '公文校对统计表';
-- Set comment to column: "c_id" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_corpid" IS '单位id';
-- Set comment to column: "c_userid" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_userid" IS '用户id';
-- Set comment to column: "c_ip" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_ip" IS 'IP';
-- Set comment to column: "n_cwsl" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."n_cwsl" IS '错误数量';
-- Set comment to column: "n_jgsl" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."n_jgsl" IS '警告数量';
-- Set comment to column: "n_tssl" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."n_tssl" IS '提示数量';
-- Set comment to column: "n_jdzs" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."n_jdzs" IS '校对字数';
-- Set comment to column: "dt_time" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."dt_time" IS '更新时间';
-- Set comment to column: "c_nyr" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_nyr" IS '年月日，格式为：yyyy年MM月dd日';
-- Set comment to column: "c_ny" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_ny" IS '年月，格式为：yyyy年MM月';
-- Set comment to column: "c_version" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_version" IS '客户端版本';
-- Set comment to column: "c_arch" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_arch" IS '客户端架构';
-- Set comment to column: "c_ostype" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_ostype" IS '客户端操作系统';
-- Set comment to column: "c_osversion" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_osversion" IS '操作系统版本';
-- Set comment to column: "c_mac" on table: "t_gwbj_tj_gwjd"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwjd"."c_mac" IS 'mac地址的md5';
-- Create "t_gwbj_tj_gwpb" table
CREATE TABLE "db_wenku"."t_gwbj_tj_gwpb" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_ip" character varying(300) NULL,
    "c_mbmc" character varying(300) NULL,
    "c_mblx" character varying(300) NULL,
    "dt_time" timestamp NULL,
    "c_nyr" character varying(300) NULL,
    "c_ny" character varying(300) NULL,
    "c_version" character varying(300) NULL,
    "c_arch" character varying(300) NULL,
    "c_ostype" character varying(300) NULL,
    "c_osversion" character varying(300) NULL,
    "c_mac" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_tj_gwpb"
COMMENT ON TABLE "db_wenku"."t_gwbj_tj_gwpb" IS '公文排版统计表';
-- Set comment to column: "c_id" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_corpid" IS '单位id';
-- Set comment to column: "c_userid" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_userid" IS '用户id';
-- Set comment to column: "c_ip" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_ip" IS 'IP';
-- Set comment to column: "c_mbmc" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_mbmc" IS '模版名称';
-- Set comment to column: "c_mblx" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_mblx" IS '模版类型';
-- Set comment to column: "dt_time" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."dt_time" IS '更新时间';
-- Set comment to column: "c_nyr" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_nyr" IS '年月日，格式为：yyyy年MM月dd日';
-- Set comment to column: "c_ny" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_ny" IS '年月，格式为：yyyy年MM月';
-- Set comment to column: "c_version" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_version" IS '客户端版本';
-- Set comment to column: "c_arch" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_arch" IS '客户端架构';
-- Set comment to column: "c_ostype" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_ostype" IS '客户端操作系统';
-- Set comment to column: "c_osversion" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_osversion" IS '操作系统版本';
-- Set comment to column: "c_mac" on table: "t_gwbj_tj_gwpb"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_gwpb"."c_mac" IS 'mac地址的md5';
-- Create "t_gwbj_tj_qt" table
CREATE TABLE "db_wenku"."t_gwbj_tj_qt" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_ip" character varying(300) NULL,
    "n_type" integer NULL,
    "n_data" bigint NULL,
    "dt_time" timestamp NULL,
    "c_nyr" character varying(300) NULL,
    "c_ny" character varying(300) NULL,
    "c_version" character varying(300) NULL,
    "c_arch" character varying(300) NULL,
    "c_ostype" character varying(300) NULL,
    "c_osversion" character varying(300) NULL,
    "c_mac" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_tj_qt"
COMMENT ON TABLE "db_wenku"."t_gwbj_tj_qt" IS '其他数据统计表';
-- Set comment to column: "c_id" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_corpid" IS '单位id';
-- Set comment to column: "c_ip" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_ip" IS 'IP';
-- Set comment to column: "n_type" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."n_type" IS '业务类型，如语音朗读、公文比对';
-- Set comment to column: "n_data" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."n_data" IS '次数或字数等数据';
-- Set comment to column: "dt_time" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."dt_time" IS '更新时间';
-- Set comment to column: "c_nyr" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_nyr" IS '年月日，格式为：yyyy年MM月dd日';
-- Set comment to column: "c_ny" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_ny" IS '年月，格式为：yyyy年MM月';
-- Set comment to column: "c_version" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_version" IS '客户端版本';
-- Set comment to column: "c_arch" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_arch" IS '客户端架构';
-- Set comment to column: "c_ostype" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_ostype" IS '客户端操作系统';
-- Set comment to column: "c_osversion" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_osversion" IS '操作系统版本';
-- Set comment to column: "c_mac" on table: "t_gwbj_tj_qt"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_qt"."c_mac" IS 'mac地址的md5';
-- Create "t_gwbj_tj_syjl" table
CREATE TABLE "db_wenku"."t_gwbj_tj_syjl" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_ip" character varying(300) NULL,
    "dt_time" timestamp NULL,
    "c_nyr" character varying(300) NULL,
    "c_ny" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_tj_syjl"
COMMENT ON TABLE "db_wenku"."t_gwbj_tj_syjl" IS '系统功能使用记录表（一个用户一天只保存一条记录）';
-- Set comment to column: "c_id" on table: "t_gwbj_tj_syjl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_syjl"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_gwbj_tj_syjl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_syjl"."c_corpid" IS '单位id';
-- Set comment to column: "c_userid" on table: "t_gwbj_tj_syjl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_syjl"."c_userid" IS '用户id';
-- Set comment to column: "c_ip" on table: "t_gwbj_tj_syjl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_syjl"."c_ip" IS 'IP';
-- Set comment to column: "dt_time" on table: "t_gwbj_tj_syjl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_syjl"."dt_time" IS '更新时间';
-- Set comment to column: "c_nyr" on table: "t_gwbj_tj_syjl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_syjl"."c_nyr" IS '年月日，格式为：yyyy年MM月dd日';
-- Set comment to column: "c_ny" on table: "t_gwbj_tj_syjl"
COMMENT ON COLUMN "db_wenku"."t_gwbj_tj_syjl"."c_ny" IS '年月，格式为：yyyy年MM月';
-- Create "t_gwbj_user_config" table
CREATE TABLE "db_wenku"."t_gwbj_user_config" (
    "c_id" character varying(300) NOT NULL,
    "c_config" text NULL,
    "dt_cjsj" timestamp NULL,
    "dt_xgsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_user_config"
COMMENT ON TABLE "db_wenku"."t_gwbj_user_config" IS '意见反馈表';
-- Set comment to column: "c_id" on table: "t_gwbj_user_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_config"."c_id" IS '编号/同用户编号';
-- Set comment to column: "c_config" on table: "t_gwbj_user_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_config"."c_config" IS '用户配置';
-- Set comment to column: "dt_cjsj" on table: "t_gwbj_user_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_config"."dt_cjsj" IS '创建时间';
-- Set comment to column: "dt_xgsj" on table: "t_gwbj_user_config"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_config"."dt_xgsj" IS '修改时间';
-- Create "t_gwbj_user_data" table
CREATE TABLE "db_wenku"."t_gwbj_user_data" (
    "c_id" character varying(32) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_loginid" character varying(300) NULL,
    "c_key" character varying(300) NULL,
    "c_value" text NULL,
    "dt_gxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_user_data"
COMMENT ON TABLE "db_wenku"."t_gwbj_user_data" IS '用户数据表';
-- Set comment to column: "c_id" on table: "t_gwbj_user_data"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_data"."c_id" IS '主键';
-- Set comment to column: "c_corpid" on table: "t_gwbj_user_data"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_data"."c_corpid" IS '用户所在单位代码';
-- Set comment to column: "c_loginid" on table: "t_gwbj_user_data"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_data"."c_loginid" IS '用户登录标识';
-- Set comment to column: "c_key" on table: "t_gwbj_user_data"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_data"."c_key" IS '关键字';
-- Set comment to column: "c_value" on table: "t_gwbj_user_data"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_data"."c_value" IS '值';
-- Set comment to column: "dt_gxsj" on table: "t_gwbj_user_data"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_data"."dt_gxsj" IS '更新时间';
-- Create "t_gwbj_user_feedback" table
CREATE TABLE "db_wenku"."t_gwbj_user_feedback" (
    "c_id" character varying(32) NOT NULL,
    "c_user_id" character varying(300) NULL,
    "c_user_nickname" character varying(300) NULL,
    "c_tenant_id" character varying(300) NULL,
    "c_tenant_name" character varying(300) NULL,
    "c_content" text NULL,
    "c_contact" character varying(300) NULL,
    "dt_cjsj" timestamp NULL,
    "c_image_path" text NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_gwbj_user_feedback"
COMMENT ON TABLE "db_wenku"."t_gwbj_user_feedback" IS '意见反馈表';
-- Set comment to column: "c_id" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."c_id" IS '编号';
-- Set comment to column: "c_user_id" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."c_user_id" IS '用户编号';
-- Set comment to column: "c_user_nickname" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."c_user_nickname" IS '用户名称';
-- Set comment to column: "c_tenant_id" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."c_tenant_id" IS '租户编号';
-- Set comment to column: "c_tenant_name" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."c_tenant_name" IS '租户名称';
-- Set comment to column: "c_content" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."c_content" IS '反馈内容';
-- Set comment to column: "c_contact" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."c_contact" IS '联系方式';
-- Set comment to column: "dt_cjsj" on table: "t_gwbj_user_feedback"
COMMENT ON COLUMN "db_wenku"."t_gwbj_user_feedback"."dt_cjsj" IS '创建时间';
-- Create "t_gwbj_xzsc" table
CREATE TABLE "db_wenku"."t_gwbj_xzsc" (
    "c_id" character varying(50) NOT NULL,
    "c_subject" character varying(300) NOT NULL,
    "c_type" character varying(100) NOT NULL,
    "b_xjp" boolean NULL,
    "c_content" text NOT NULL,
    "c_source" character varying(500) NULL,
    "c_example" text NULL,
    "c_allusion" text NULL,
    "c_classify" character varying(1000) NULL,
    "c_explain" text NULL,
    "c_appreciation" text NULL,
    "c_usage" character varying(1000) NULL,
    "c_fonttype" character varying(300) NULL,
    "dt_fbrq" timestamp NULL,
    "n_frequency" integer NULL,
    "dt_fwrq" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "t_gwbj_xzsc_c_type_index" to table: "t_gwbj_xzsc"
CREATE INDEX "t_gwbj_xzsc_c_type_index" ON "db_wenku"."t_gwbj_xzsc" ("c_type");
-- Set comment to table: "t_gwbj_xzsc"
COMMENT ON TABLE "db_wenku"."t_gwbj_xzsc" IS '写作素材表';
-- Set comment to column: "c_id" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_id" IS 'id';
-- Set comment to column: "c_subject" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_subject" IS '主题';
-- Set comment to column: "c_type" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_type" IS '类型，用典/金句/结构/用词/习近平讲话';
-- Set comment to column: "b_xjp" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."b_xjp" IS '是否习近平';
-- Set comment to column: "c_content" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_content" IS '内容';
-- Set comment to column: "c_source" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_source" IS '来源';
-- Set comment to column: "c_example" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_example" IS '例文';
-- Set comment to column: "c_allusion" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_allusion" IS '典故';
-- Set comment to column: "c_classify" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_classify" IS '类别';
-- Set comment to column: "c_explain" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_explain" IS '释义';
-- Set comment to column: "c_appreciation" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_appreciation" IS '赏析';
-- Set comment to column: "c_usage" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_usage" IS '用法';
-- Set comment to column: "c_fonttype" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."c_fonttype" IS '字形';
-- Set comment to column: "dt_fbrq" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."dt_fbrq" IS '发布日期';
-- Set comment to column: "n_frequency" on table: "t_gwbj_xzsc"
COMMENT ON COLUMN "db_wenku"."t_gwbj_xzsc"."n_frequency" IS '使用次数';
-- Create "t_label_map" table
CREATE TABLE "db_wenku"."t_label_map" (
    "c_id" bigserial NOT NULL,
    "c_label_name" character varying(100) NOT NULL,
    "c_label_id" integer NOT NULL,
    "c_placeholder" character varying(200) NULL,
    "c_group_name" character varying(50) NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_learning_hub" table
CREATE TABLE "db_wenku"."t_learning_hub" (
    "c_id" character varying(50) NOT NULL,
    "c_type" character varying(100) NULL,
    "c_title" text NULL,
    "c_content" text NULL,
    "c_file_type" character varying(20) NULL,
    "c_src_path" character varying(255) NULL,
    "c_pdf_path" character varying(255) NULL,
    "n_is_top" integer NULL DEFAULT 0,
    "dt_create_time" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY ("c_id")
);
-- Create index "idx_learning_hub_create_time" to table: "t_learning_hub"
CREATE INDEX "idx_learning_hub_create_time" ON "db_wenku"."t_learning_hub" ("dt_create_time");
-- Create index "idx_learning_hub_type" to table: "t_learning_hub"
CREATE INDEX "idx_learning_hub_type" ON "db_wenku"."t_learning_hub" ("c_type");
-- Set comment to table: "t_learning_hub"
COMMENT ON TABLE "db_wenku"."t_learning_hub" IS '学习中心表';
-- Set comment to column: "c_id" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."c_id" IS '主键ID';
-- Set comment to column: "c_type" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."c_type" IS '类型';
-- Set comment to column: "c_title" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."c_title" IS '标题';
-- Set comment to column: "c_content" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."c_content" IS '内容';
-- Set comment to column: "c_file_type" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."c_file_type" IS '文件后缀';
-- Set comment to column: "c_src_path" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."c_src_path" IS '原文件MinIO路径';
-- Set comment to column: "c_pdf_path" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."c_pdf_path" IS 'PDF MinIO路径';
-- Set comment to column: "n_is_top" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."n_is_top" IS '是否置顶：1置顶，0不置顶';
-- Set comment to column: "dt_create_time" on table: "t_learning_hub"
COMMENT ON COLUMN "db_wenku"."t_learning_hub"."dt_create_time" IS '创建时间';
-- Create "t_license" table
CREATE TABLE "db_wenku"."t_license" (
    "c_id" character varying(50) NOT NULL,
    "c_key" character varying(50) NULL,
    "c_content" text NULL,
    "c_description" text NULL,
    "dt_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_llm_config" table
CREATE TABLE "db_wenku"."t_llm_config" (
    "c_id" character varying(50) NOT NULL,
    "c_key" character varying(300) NULL,
    "c_value" text NULL,
    "c_description" text NULL,
    "n_type" integer NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_lxxx" table
CREATE TABLE "db_wenku"."t_lxxx" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_phone" character varying(300) NOT NULL,
    "c_mail" character varying(300) NOT NULL,
    "c_industry" character varying(300) NULL,
    "c_requirement" text NOT NULL,
    "c_company" character varying(300) NULL,
    "c_architecture" character varying(100) NOT NULL,
    "c_submit_time" character varying(100) NOT NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_t_lxxx_submit_time" to table: "t_lxxx"
CREATE INDEX "i_t_lxxx_submit_time" ON "db_wenku"."t_lxxx" ("c_submit_time");
-- Set comment to column: "c_id" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_id" IS 'id';
-- Set comment to column: "c_name" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_name" IS '姓名';
-- Set comment to column: "c_phone" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_phone" IS '电话';
-- Set comment to column: "c_mail" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_mail" IS '邮箱';
-- Set comment to column: "c_industry" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_industry" IS '行业';
-- Set comment to column: "c_requirement" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_requirement" IS '业务需求';
-- Set comment to column: "c_company" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_company" IS '公司';
-- Set comment to column: "c_architecture" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_architecture" IS '架构';
-- Set comment to column: "c_submit_time" on table: "t_lxxx"
COMMENT ON COLUMN "db_wenku"."t_lxxx"."c_submit_time" IS '提交时间';
-- Create "t_notice" table
CREATE TABLE "db_wenku"."t_notice" (
    "c_id" character varying(50) NOT NULL,
    "c_type" character varying(100) NULL,
    "c_content" text NULL,
    "c_clientversion" character varying(100) NULL,
    "n_valid" integer NULL,
    "dt_createtime" timestamp NULL,
    "c_corpid" character varying(300) NULL,
    "dt_expiretime" timestamp NULL,
    "c_title" character varying(300) NULL,
    "n_popup" integer NULL,
    "c_ext" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_notice_record" table
CREATE TABLE "db_wenku"."t_notice_record" (
    "c_id" character varying(50) NOT NULL,
    "c_loginid" character varying(100) NULL,
    "c_noticeid" character varying(100) NULL,
    "c_corpid" character varying(100) NULL,
    "n_valid" integer NULL,
    "n_rstatus" integer NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_notice_record_loginid" to table: "t_notice_record"
CREATE INDEX "i_notice_record_loginid" ON "db_wenku"."t_notice_record" ("c_loginid");
-- Create "t_open_lyzj_record" table
CREATE TABLE "db_wenku"."t_open_lyzj_record" (
    "c_id" character varying(50) NOT NULL,
    "c_file_name" character varying(300) NULL,
    "n_state" integer NULL,
    "c_summary" text NULL,
    "c_error_msg" character varying(1000) NULL,
    "c_client_ip" character varying(300) NULL,
    "dt_create_time" timestamp NULL,
    "dt_finish_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_open_lyzj_record_create_time" to table: "t_open_lyzj_record"
CREATE INDEX "i_open_lyzj_record_create_time" ON "db_wenku"."t_open_lyzj_record" ("dt_create_time");
-- Create index "i_open_lyzj_record_state" to table: "t_open_lyzj_record"
CREATE INDEX "i_open_lyzj_record_state" ON "db_wenku"."t_open_lyzj_record" ("n_state");
-- Set comment to table: "t_open_lyzj_record"
COMMENT ON TABLE "db_wenku"."t_open_lyzj_record" IS '第三方会议智记开放接口调用记录表';
-- Set comment to column: "c_id" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."c_id" IS '任务ID，对外暴露的audioId';
-- Set comment to column: "c_file_name" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."c_file_name" IS '上传文件名';
-- Set comment to column: "n_state" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."n_state" IS '状态：0处理中，1成功，2失败';
-- Set comment to column: "c_summary" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."c_summary" IS '生成的结构化纪要JSON';
-- Set comment to column: "c_error_msg" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."c_error_msg" IS '失败原因';
-- Set comment to column: "c_client_ip" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."c_client_ip" IS '调用方IP';
-- Set comment to column: "dt_create_time" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."dt_create_time" IS '创建时间';
-- Set comment to column: "dt_finish_time" on table: "t_open_lyzj_record"
COMMENT ON COLUMN "db_wenku"."t_open_lyzj_record"."dt_finish_time" IS '完成时间';
-- Create "t_operate_log" table
CREATE TABLE "db_wenku"."t_operate_log" (
    "c_id" character varying(255) NOT NULL,
    "dt_create_time" timestamp NULL,
    "c_error_message" text NULL,
    "c_file_size" character varying(255) NULL,
    "c_ip" character varying(255) NULL,
    "c_operation_content" text NULL,
    "c_operation_type" character varying(255) NULL,
    "n_success" boolean NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_outer_corp" table
CREATE TABLE "db_wenku"."t_outer_corp" (
    "c_id" character varying(50) NOT NULL,
    "c_parentid" character varying(300) NULL,
    "c_parentcode" character varying(300) NULL,
    "c_parentname" character varying(300) NULL,
    "c_corpid" character varying(300) NULL,
    "c_corpcode" character varying(300) NULL,
    "c_corpname" character varying(300) NULL,
    "c_lxr" character varying(300) NULL,
    "dt_cjsj" timestamp NULL,
    "dt_gxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_id" IS '编号';
-- Set comment to column: "c_parentid" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_parentid" IS '上级机构id';
-- Set comment to column: "c_parentcode" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_parentcode" IS '上级机构代码';
-- Set comment to column: "c_parentname" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_parentname" IS '上级机构名称';
-- Set comment to column: "c_corpid" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_corpid" IS '单位id';
-- Set comment to column: "c_corpcode" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_corpcode" IS '单位代码';
-- Set comment to column: "c_corpname" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_corpname" IS '单位名称';
-- Set comment to column: "c_lxr" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."c_lxr" IS '联系人';
-- Set comment to column: "dt_cjsj" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."dt_cjsj" IS '创建时间';
-- Set comment to column: "dt_gxsj" on table: "t_outer_corp"
COMMENT ON COLUMN "db_wenku"."t_outer_corp"."dt_gxsj" IS '更新时间';
-- Create "t_outer_user" table
CREATE TABLE "db_wenku"."t_outer_user" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_corpcode" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_loginid" character varying(300) NULL,
    "c_nickname" character varying(300) NULL,
    "dt_zcsj" timestamp NULL,
    "dt_gxsj" timestamp NULL,
    "c_ny" character varying(100) NULL,
    "c_nyr" character varying(100) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_outer_user"
COMMENT ON COLUMN "db_wenku"."t_outer_user"."c_id" IS '编号';
-- Set comment to column: "c_corpid" on table: "t_outer_user"
COMMENT ON COLUMN "db_wenku"."t_outer_user"."c_corpid" IS '单位id';
-- Set comment to column: "c_corpcode" on table: "t_outer_user"
COMMENT ON COLUMN "db_wenku"."t_outer_user"."c_corpcode" IS '单位代码';
-- Set comment to column: "c_userid" on table: "t_outer_user"
COMMENT ON COLUMN "db_wenku"."t_outer_user"."c_userid" IS '用户id';
-- Set comment to column: "c_loginid" on table: "t_outer_user"
COMMENT ON COLUMN "db_wenku"."t_outer_user"."c_loginid" IS '用户登录标识';
-- Set comment to column: "dt_zcsj" on table: "t_outer_user"
COMMENT ON COLUMN "db_wenku"."t_outer_user"."dt_zcsj" IS '注册时间';
-- Set comment to column: "dt_gxsj" on table: "t_outer_user"
COMMENT ON COLUMN "db_wenku"."t_outer_user"."dt_gxsj" IS '更新时间';
-- Create "t_print_config" table
CREATE TABLE "db_wenku"."t_print_config" (
    "c_id" bigserial NOT NULL,
    "c_name" character varying(256) NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_qc_detail" table
CREATE TABLE "db_wenku"."t_qc_detail" (
    "c_id" character varying(32) NOT NULL,
    "c_taskid" character varying(32) NULL,
    "c_loginid" character varying(300) NULL,
    "c_filename" character varying(300) NULL,
    "c_filepath" character varying(300) NULL,
    "c_filecontent" text NULL,
    "c_title" character varying(300) NULL,
    "c_fwzh" character varying(300) NULL,
    "c_fwdw" character varying(300) NULL,
    "c_errorresult" text NULL,
    "n_errorcount" integer NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_qc_detail_loginid" to table: "t_qc_detail"
CREATE INDEX "i_qc_detail_loginid" ON "db_wenku"."t_qc_detail" ("c_loginid");
-- Create index "i_qc_detail_taskid" to table: "t_qc_detail"
CREATE INDEX "i_qc_detail_taskid" ON "db_wenku"."t_qc_detail" ("c_taskid");
-- Create "t_qc_task" table
CREATE TABLE "db_wenku"."t_qc_task" (
    "c_id" character varying(32) NOT NULL,
    "c_loginid" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "dt_createtime" timestamp NULL,
    "n_filecount" integer NULL,
    "n_state" integer NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_qc_task_loginid" to table: "t_qc_task"
CREATE INDEX "i_qc_task_loginid" ON "db_wenku"."t_qc_task" ("c_loginid");
-- Create index "i_qc_task_name" to table: "t_qc_task"
CREATE INDEX "i_qc_task_name" ON "db_wenku"."t_qc_task" ("c_name");
-- Create "t_qr_code_config" table
CREATE TABLE "db_wenku"."t_qr_code_config" (
    "c_id" character varying(64) NOT NULL,
    "c_size_type" character varying(32) NULL,
    "c_custom_width" numeric(10,2) NULL,
    "c_custom_height" numeric(10,2) NULL,
    "c_position_type" character varying(32) NULL,
    "c_horizontal_value" numeric(10,2) NULL,
    "c_horizontal_type" character varying(32) NULL,
    "c_vertical_value" numeric(10,2) NULL,
    "c_vertical_type" character varying(32) NULL,
    "c_level1_name" character varying(128) NULL,
    "c_level1_code" character varying(64) NULL,
    "c_level2_name" character varying(128) NULL,
    "c_level2_code" character varying(64) NULL,
    "c_level3_name" character varying(128) NULL,
    "c_level3_code" character varying(64) NULL,
    "c_level4_name" character varying(128) NULL,
    "c_level4_code" character varying(64) NULL,
    "c_user_code" character varying(64) NULL,
    "c_user_name" character varying(64) NULL,
    "c_phone" character varying(32) NULL,
    "c_user_id" character varying(64) NULL,
    "c_token" character varying(256) NULL,
    "dt_create" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    "dt_update" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY ("c_id")
);
-- Create "t_qr_code_unit_list" table
CREATE TABLE "db_wenku"."t_qr_code_unit_list" (
    "c_id" character varying(64) NOT NULL,
    "c_config_id" character varying(64) NOT NULL,
    "c_unit_type" character varying(32) NULL,
    "c_unit_id" bigint NULL,
    "c_unit_name" character varying(128) NULL,
    "n_order" integer NULL,
    "dt_create" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    "dt_update" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY ("c_id")
);
-- Create "t_replaceword" table
CREATE TABLE "db_wenku"."t_replaceword" (
    "c_id" character varying(32) NOT NULL,
    "c_key" character varying(300) NULL,
    "c_value" character varying(300) NULL,
    "n_retain" integer NULL DEFAULT 0,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_replaceword"
COMMENT ON COLUMN "db_wenku"."t_replaceword"."c_id" IS '编号';
-- Set comment to column: "c_key" on table: "t_replaceword"
COMMENT ON COLUMN "db_wenku"."t_replaceword"."c_key" IS '替换旧值';
-- Set comment to column: "c_value" on table: "t_replaceword"
COMMENT ON COLUMN "db_wenku"."t_replaceword"."c_value" IS '替换新值';
-- Set comment to column: "n_retain" on table: "t_replaceword"
COMMENT ON COLUMN "db_wenku"."t_replaceword"."n_retain" IS '是否保留';
-- Create "t_require" table
CREATE TABLE "db_wenku"."t_require" (
    "c_id" character varying(32) NOT NULL,
    "c_key" character varying(300) NULL,
    "c_content" text NULL,
    "n_retain" integer NULL DEFAULT 0,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_require"
COMMENT ON COLUMN "db_wenku"."t_require"."c_id" IS '编号';
-- Set comment to column: "c_key" on table: "t_require"
COMMENT ON COLUMN "db_wenku"."t_require"."c_key" IS '写作类型';
-- Set comment to column: "c_content" on table: "t_require"
COMMENT ON COLUMN "db_wenku"."t_require"."c_content" IS '要求内容';
-- Set comment to column: "n_retain" on table: "t_require"
COMMENT ON COLUMN "db_wenku"."t_require"."n_retain" IS '是否保留';
-- Create "t_research_content" table
CREATE TABLE "db_wenku"."t_research_content" (
    "c_id" character varying(64) NOT NULL,
    "c_task_id" character varying(64) NOT NULL,
    "c_status" character varying(32) NULL,
    "c_user_input" text NULL,
    "dt_create_time" timestamp NULL,
    "dt_completed_time" timestamp NULL,
    "c_content" text NULL,
    "c_writer" text NULL,
    "c_extra_data" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_research_share" table
CREATE TABLE "db_wenku"."t_research_share" (
    "c_share_id" character varying(64) NOT NULL,
    "c_task_id" character varying(150) NOT NULL,
    "c_login_id" character varying(64) NOT NULL,
    "c_share_title" character varying(500) NULL,
    "c_share_type" character varying(32) NULL DEFAULT 'public',
    "c_password" character varying(128) NULL,
    "c_expire_time" timestamp NULL,
    "c_access_count" integer NULL DEFAULT 0,
    "c_max_access_count" integer NULL,
    "c_status" character varying(32) NULL DEFAULT 'active',
    "c_extra_data" text NULL,
    "dt_create_time" timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY ("c_share_id")
);
-- Create "t_research_tasks" table
CREATE TABLE "db_wenku"."t_research_tasks" (
    "c_task_id" character varying(150) NOT NULL,
    "c_login_id" character varying(64) NOT NULL,
    "c_status" character varying(32) NOT NULL DEFAULT '进行中',
    "c_task_name" text NULL,
    "c_user_input" text NULL,
    "dt_create_time" timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "c_extra_data" text NULL,
    CONSTRAINT "pk_research_tasks" PRIMARY KEY ("c_task_id")
);
-- Create "t_role" table
CREATE TABLE "db_wenku"."t_role" (
    "c_id" character varying(32) NOT NULL,
    "c_type" character varying(300) NULL,
    "c_subtype" character varying(300) NULL,
    "c_template" text NULL,
    "n_retain" integer NULL DEFAULT 0,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_role"
COMMENT ON COLUMN "db_wenku"."t_role"."c_id" IS '编号';
-- Set comment to column: "c_type" on table: "t_role"
COMMENT ON COLUMN "db_wenku"."t_role"."c_type" IS '写作类型';
-- Set comment to column: "c_subtype" on table: "t_role"
COMMENT ON COLUMN "db_wenku"."t_role"."c_subtype" IS '子类型';
-- Set comment to column: "c_template" on table: "t_role"
COMMENT ON COLUMN "db_wenku"."t_role"."c_template" IS '提示词模板';
-- Set comment to column: "n_retain" on table: "t_role"
COMMENT ON COLUMN "db_wenku"."t_role"."n_retain" IS '是否保留';
-- Create "t_score_item" table
CREATE TABLE "db_wenku"."t_score_item" (
    "id" integer NOT NULL,
    "pid" integer NOT NULL,
    "c_type" character varying(50) NOT NULL,
    "c_item" text NULL,
    "c_score" text NULL,
    "c_desc" text NULL,
    PRIMARY KEY ("id")
);
-- Create index "i_score_item_pid" to table: "t_score_item"
CREATE INDEX "i_score_item_pid" ON "db_wenku"."t_score_item" ("pid");
-- Create "t_sensitive_word" table
CREATE TABLE "db_wenku"."t_sensitive_word" (
    "c_id" character varying(32) NOT NULL,
    "c_key" character varying(300) NULL,
    "c_value" character varying(300) NULL,
    "n_retain" integer NULL DEFAULT 0,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_sensitive_word"
COMMENT ON COLUMN "db_wenku"."t_sensitive_word"."c_id" IS '编号';
-- Set comment to column: "c_key" on table: "t_sensitive_word"
COMMENT ON COLUMN "db_wenku"."t_sensitive_word"."c_key" IS '敏感词';
-- Set comment to column: "c_value" on table: "t_sensitive_word"
COMMENT ON COLUMN "db_wenku"."t_sensitive_word"."c_value" IS '替换词';
-- Set comment to column: "n_retain" on table: "t_sensitive_word"
COMMENT ON COLUMN "db_wenku"."t_sensitive_word"."n_retain" IS '是否保留';
-- Create "t_session" table
CREATE TABLE "db_wenku"."t_session" (
    "c_id" character varying(50) NOT NULL,
    "c_sid" character varying(300) NULL,
    "b_session" bytea NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_t_session" to table: "t_session"
CREATE INDEX "i_t_session" ON "db_wenku"."t_session" ("c_sid");
-- Create "t_snpt_doctext" table
CREATE TABLE "db_wenku"."t_snpt_doctext" (
    "c_id" character varying(50) NOT NULL,
    "c_docid" character varying(300) NOT NULL,
    "c_textid" character varying(300) NOT NULL,
    "c_title" character varying(300) NOT NULL,
    "c_text" text NOT NULL,
    "c_doctype" character varying(50) NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "c_docid" to table: "t_snpt_doctext"
CREATE INDEX "c_docid" ON "db_wenku"."t_snpt_doctext" ("c_docid");
-- Create index "c_textid" to table: "t_snpt_doctext"
CREATE INDEX "c_textid" ON "db_wenku"."t_snpt_doctext" ("c_textid");
-- Set comment to column: "c_id" on table: "t_snpt_doctext"
COMMENT ON COLUMN "db_wenku"."t_snpt_doctext"."c_id" IS 'id';
-- Set comment to column: "c_docid" on table: "t_snpt_doctext"
COMMENT ON COLUMN "db_wenku"."t_snpt_doctext"."c_docid" IS '素材id';
-- Set comment to column: "c_textid" on table: "t_snpt_doctext"
COMMENT ON COLUMN "db_wenku"."t_snpt_doctext"."c_textid" IS '文本id';
-- Set comment to column: "c_title" on table: "t_snpt_doctext"
COMMENT ON COLUMN "db_wenku"."t_snpt_doctext"."c_title" IS '文章标题';
-- Set comment to column: "c_text" on table: "t_snpt_doctext"
COMMENT ON COLUMN "db_wenku"."t_snpt_doctext"."c_text" IS '文章内容';
-- Create "t_snpt_favorite" table
CREATE TABLE "db_wenku"."t_snpt_favorite" (
    "c_id" character varying(50) NOT NULL,
    "c_docid" character varying(300) NOT NULL,
    "c_userid" character varying(300) NOT NULL,
    "c_textid" character varying(300) NOT NULL,
    "c_time" character varying(100) NOT NULL,
    "c_doctype" character varying(50) NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "c_userid" to table: "t_snpt_favorite"
CREATE INDEX "c_userid" ON "db_wenku"."t_snpt_favorite" ("c_userid");
-- Set comment to column: "c_id" on table: "t_snpt_favorite"
COMMENT ON COLUMN "db_wenku"."t_snpt_favorite"."c_id" IS 'id';
-- Set comment to column: "c_docid" on table: "t_snpt_favorite"
COMMENT ON COLUMN "db_wenku"."t_snpt_favorite"."c_docid" IS '素材id';
-- Set comment to column: "c_userid" on table: "t_snpt_favorite"
COMMENT ON COLUMN "db_wenku"."t_snpt_favorite"."c_userid" IS '用户id';
-- Set comment to column: "c_textid" on table: "t_snpt_favorite"
COMMENT ON COLUMN "db_wenku"."t_snpt_favorite"."c_textid" IS '内容id';
-- Set comment to column: "c_time" on table: "t_snpt_favorite"
COMMENT ON COLUMN "db_wenku"."t_snpt_favorite"."c_time" IS '创建时间';
-- Create "t_snpt_zsk_rel" table
CREATE TABLE "db_wenku"."t_snpt_zsk_rel" (
    "c_id" character varying(50) NOT NULL,
    "c_zsk_id" character varying(300) NULL,
    "c_zsk_code" character varying(300) NULL,
    "c_rel_id" character varying(300) NULL,
    "c_rel_type" character varying(300) NULL,
    CONSTRAINT "t_snpt_zsk_rel_key" PRIMARY KEY ("c_id")
);
-- Create "t_special_topic" table
CREATE TABLE "db_wenku"."t_special_topic" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(100) NULL,
    "c_overview_image" text NULL,
    "c_overview_sm_image" text NULL,
    "c_rotation_images" text NULL,
    "c_description" text NULL,
    "n_enable" integer NULL DEFAULT 0,
    "dt_createtime" timestamp NULL,
    "c_plate" text NULL,
    "n_order" integer NULL DEFAULT 1,
    CONSTRAINT "t_special_topic_pk" PRIMARY KEY ("c_id")
);
-- Create "t_sysconfig" table
CREATE TABLE "db_wenku"."t_sysconfig" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NULL,
    "c_key" character varying(100) NULL,
    "c_value" character varying(300) NULL,
    "n_order" integer NULL,
    "dt_createtime" timestamp NULL,
    "dt_updatetime" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_sysconfig"
COMMENT ON TABLE "db_wenku"."t_sysconfig" IS '全局配置表';
-- Set comment to column: "c_id" on table: "t_sysconfig"
COMMENT ON COLUMN "db_wenku"."t_sysconfig"."c_id" IS 'id';
-- Set comment to column: "c_name" on table: "t_sysconfig"
COMMENT ON COLUMN "db_wenku"."t_sysconfig"."c_name" IS '配置项名称';
-- Set comment to column: "c_key" on table: "t_sysconfig"
COMMENT ON COLUMN "db_wenku"."t_sysconfig"."c_key" IS '配置项关键字';
-- Set comment to column: "c_value" on table: "t_sysconfig"
COMMENT ON COLUMN "db_wenku"."t_sysconfig"."c_value" IS '显示项关键字';
-- Set comment to column: "n_order" on table: "t_sysconfig"
COMMENT ON COLUMN "db_wenku"."t_sysconfig"."n_order" IS '显示顺序';
-- Set comment to column: "dt_createtime" on table: "t_sysconfig"
COMMENT ON COLUMN "db_wenku"."t_sysconfig"."dt_createtime" IS '创建时间';
-- Set comment to column: "dt_updatetime" on table: "t_sysconfig"
COMMENT ON COLUMN "db_wenku"."t_sysconfig"."dt_updatetime" IS '更新时间';
-- Create "t_template" table
CREATE TABLE "db_wenku"."t_template" (
    "c_id" character varying(32) NOT NULL,
    "c_type" character varying(300) NULL,
    "c_subtype" character varying(300) NULL,
    "c_template" text NULL,
    "n_retain" integer NULL DEFAULT 0,
    "c_title" character varying(300) NULL,
    "dt_create" timestamp NULL,
    "dt_lastmodify" timestamp NULL,
    "n_rule" character varying(20) NULL DEFAULT 'and',
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_template"
COMMENT ON COLUMN "db_wenku"."t_template"."c_id" IS '编号';
-- Set comment to column: "c_type" on table: "t_template"
COMMENT ON COLUMN "db_wenku"."t_template"."c_type" IS '写作类型';
-- Set comment to column: "c_subtype" on table: "t_template"
COMMENT ON COLUMN "db_wenku"."t_template"."c_subtype" IS '子类型';
-- Set comment to column: "c_template" on table: "t_template"
COMMENT ON COLUMN "db_wenku"."t_template"."c_template" IS '提示词模板';
-- Set comment to column: "n_retain" on table: "t_template"
COMMENT ON COLUMN "db_wenku"."t_template"."n_retain" IS '是否保留';
-- Create "t_template_ban" table
CREATE TABLE "db_wenku"."t_template_ban" (
    "c_id" character varying(40) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_file_path" character varying(900) NULL,
    "c_create_time" bigint NOT NULL,
    "n_order" integer NULL DEFAULT 0,
    PRIMARY KEY ("c_id")
);
-- Create "t_template_edit_config" table
CREATE TABLE "db_wenku"."t_template_edit_config" (
    "c_id" bigserial NOT NULL,
    "c_group_name" character varying(20) NULL,
    "c_component_type" character varying(50) NULL,
    "c_label_name" character varying(20) NULL,
    "c_label_id" integer NULL,
    "c_row" integer NULL,
    "c_col" integer NULL,
    "c_item_order" integer NULL,
    "c_flex" character varying(20) NULL,
    "c_is_show" boolean NULL DEFAULT true,
    "c_is_tick" boolean NULL DEFAULT true,
    "c_group_id" integer NULL DEFAULT 1,
    "c_gen_label_name" character varying(25) NULL,
    "c_api_value" character varying(50) NULL,
    "c_attr_value" character varying(100) NULL,
    "c_placeholder" character varying(20) NULL,
    "c_gen_attr_value" character varying(30) NULL,
    "c_template_ban_id" character varying(50) NULL,
    "c_bookmark" character varying(10) NULL,
    "c_default_value" character varying(100) NULL,
    "c_length" integer NULL,
    "c_add_content" character varying(10) NULL,
    "c_add_position" character varying(10) NULL,
    "c_align" character varying(10) NULL,
    "c_indent" numeric(10) NULL,
    "c_font_style" character varying(50) NULL,
    "c_font_color" character varying(20) NULL,
    "c_font_size" character varying(10) NULL,
    "c_border_top" character varying(20) NULL,
    "c_border_bottom" character varying(20) NULL,
    "c_left_indent" numeric(10,2) NULL DEFAULT 0,
    "c_right_indent" numeric(10,2) NULL DEFAULT 0,
    PRIMARY KEY ("c_id")
);
-- Create "t_template_edit_setting" table
CREATE TABLE "db_wenku"."t_template_edit_setting" (
    "c_id" character varying(50) NOT NULL,
    "c_config_key" character varying(100) NOT NULL,
    "c_config_value" character varying(500) NULL,
    "c_config_type" character varying(20) NOT NULL,
    "c_description" character varying(200) NULL,
    "c_create_time" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    "c_update_time" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY ("c_id"),
    CONSTRAINT "t_template_edit_setting_c_config_key_key" UNIQUE ("c_config_key")
);
-- Create "t_template_value" table
CREATE TABLE "db_wenku"."t_template_value" (
    "c_id" bigserial NOT NULL,
    "c_label_id" integer NOT NULL,
    "c_attr_value" character varying(500) NULL,
    "c_type" character varying(50) NULL,
    "c_default_value" character varying(2000) NULL,
    "c_gen_default_value" character varying(100) NULL,
    "c_is_show" boolean NOT NULL DEFAULT true,
    "c_is_tick" boolean NOT NULL DEFAULT true,
    "c_is_gen_tick" boolean NOT NULL DEFAULT true,
    "c_unit_default_value" character varying(3) NULL,
    "c_template_id" character varying(50) NULL,
    "c_ban_id" character varying(50) NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_tj_gwjs" table
CREATE TABLE "db_wenku"."t_tj_gwjs" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_ip" character varying(300) NULL,
    "n_type" integer NULL,
    "dt_time" timestamp NULL,
    "c_nyr" character varying(300) NULL,
    "c_ny" character varying(300) NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_tj_gwjs_corpid" to table: "t_tj_gwjs"
CREATE INDEX "i_tj_gwjs_corpid" ON "db_wenku"."t_tj_gwjs" ("c_corpid");
-- Create "t_tj_user_operate" table
CREATE TABLE "db_wenku"."t_tj_user_operate" (
    "c_id" character varying(50) NOT NULL,
    "c_userid" character varying(300) NULL,
    "c_corpid" character varying(300) NULL,
    "c_ip" character varying(300) NULL,
    "n_type" integer NULL,
    "c_xznr" text NULL,
    "dt_time" timestamp NULL,
    "c_nyr" character varying(100) NULL,
    "c_ny" character varying(100) NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "t_user_operate_c_corpid_index" to table: "t_tj_user_operate"
CREATE INDEX "t_user_operate_c_corpid_index" ON "db_wenku"."t_tj_user_operate" ("c_corpid");
-- Create index "t_user_operate_c_userid_index" to table: "t_tj_user_operate"
CREATE INDEX "t_user_operate_c_userid_index" ON "db_wenku"."t_tj_user_operate" ("c_userid");
-- Create "t_tj_wdgj" table
CREATE TABLE "db_wenku"."t_tj_wdgj" (
    "c_id" character varying(50) NOT NULL,
    "c_filename" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "c_filepath" character varying(300) NULL,
    "c_filelabel" character varying(50) NULL,
    "c_filetype" character varying(20) NULL,
    "c_abbrcontent" character varying(600) NULL,
    "n_viewcount" integer NULL,
    "n_filelength" integer NULL,
    "n_wordcount" integer NULL,
    "n_storagetype" integer NULL,
    "n_filestatus" integer NULL,
    "dt_ctime" timestamp NULL,
    "dt_utime" timestamp NULL,
    "c_catagory" character varying(300) NULL,
    "c_labels" character varying(300) NULL,
    "n_encrypt" integer NULL,
    "c_dir_id" character varying(50) NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_tj_wdgj_id" to table: "t_tj_wdgj"
CREATE INDEX "i_tj_wdgj_id" ON "db_wenku"."t_tj_wdgj" ("c_id");
-- Create "t_tj_wdgj_dir" table
CREATE TABLE "db_wenku"."t_tj_wdgj_dir" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_userid" character varying(300) NOT NULL,
    "c_parent_id" character varying(50) NULL,
    "dt_ctime" timestamp NULL,
    "dt_utime" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "i_tj_wdgj_dir_userid" to table: "t_tj_wdgj_dir"
CREATE INDEX "i_tj_wdgj_dir_userid" ON "db_wenku"."t_tj_wdgj_dir" ("c_userid");
-- Create "t_tjfx" table
CREATE TABLE "db_wenku"."t_tjfx" (
    "c_id" character varying(50) NOT NULL,
    "c_ip" character varying(300) NULL,
    "c_gn" character varying(300) NULL,
    "c_mbmc" character varying(300) NULL,
    "c_mblx" character varying(300) NULL,
    "c_mj" character varying(300) NULL,
    "n_cwsl" integer NULL,
    "n_jgsl" integer NULL,
    "n_tssl" integer NULL,
    "n_jdzs" integer NULL,
    "dt_gxsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_tjfx"
COMMENT ON TABLE "db_wenku"."t_tjfx" IS '统计分析表';
-- Set comment to column: "c_id" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."c_id" IS 'id';
-- Set comment to column: "c_ip" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."c_ip" IS '用户ip';
-- Set comment to column: "c_gn" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."c_gn" IS '用户使用的功能(1 校对 0 排版 2 智能写作)';
-- Set comment to column: "c_mbmc" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."c_mbmc" IS '模板名称';
-- Set comment to column: "c_mblx" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."c_mblx" IS '模板类型';
-- Set comment to column: "c_mj" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."c_mj" IS '密级';
-- Set comment to column: "n_cwsl" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."n_cwsl" IS '错误数量';
-- Set comment to column: "n_jgsl" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."n_jgsl" IS '警告数量';
-- Set comment to column: "n_tssl" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."n_tssl" IS '提示数量';
-- Set comment to column: "dt_gxsj" on table: "t_tjfx"
COMMENT ON COLUMN "db_wenku"."t_tjfx"."dt_gxsj" IS '更新时间';
-- Create "t_unit_person" table
CREATE TABLE "db_wenku"."t_unit_person" (
    "c_id" bigint NOT NULL GENERATED ALWAYS AS IDENTITY,
    "c_unit_person_id" character varying(40) NOT NULL,
    "c_unit_name" character varying(100) NOT NULL,
    "n_order" integer NULL DEFAULT 0,
    "c_create_time" bigint NOT NULL,
    "c_remark" character varying(200) NULL,
    "c_is_tick" boolean NULL,
    "c_group_id" bigint NULL,
    "c_common_group_id" bigint NULL,
    PRIMARY KEY ("c_id"),
    CONSTRAINT "uk_unit_person_id" UNIQUE ("c_unit_person_id")
);
-- Create index "i_unit_person_group_id" to table: "t_unit_person"
CREATE INDEX "i_unit_person_group_id" ON "db_wenku"."t_unit_person" ("c_group_id");
-- Create "t_unit_person_group" table
CREATE TABLE "db_wenku"."t_unit_person_group" (
    "c_group_id" bigserial NOT NULL,
    "c_group_name" character varying(100) NOT NULL,
    "n_order" integer NULL DEFAULT 0,
    PRIMARY KEY ("c_group_id")
);
-- Create index "i_unit_person_group_order" to table: "t_unit_person_group"
CREATE INDEX "i_unit_person_group_order" ON "db_wenku"."t_unit_person_group" ("n_order");
-- Create "t_upfile" table
CREATE TABLE "db_wenku"."t_upfile" (
    "c_id" character varying(32) NOT NULL,
    "c_name" character varying(300) NULL,
    "c_filename" character varying(300) NULL,
    "c_type" character varying(300) NULL,
    "n_status" integer NULL,
    "dt_time" timestamp NULL,
    "c_groupid" character varying(50) NULL,
    "n_order" integer NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to column: "c_id" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."c_id" IS '主键';
-- Set comment to column: "c_name" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."c_name" IS '模板名称';
-- Set comment to column: "c_filename" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."c_filename" IS '模板文件名';
-- Set comment to column: "c_type" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."c_type" IS '模版类型：default、import';
-- Set comment to column: "n_status" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."n_status" IS '状态';
-- Set comment to column: "dt_time" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."dt_time" IS '更新时间';
-- Set comment to column: "c_groupid" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."c_groupid" IS '分组id';
-- Set comment to column: "n_order" on table: "t_upfile"
COMMENT ON COLUMN "db_wenku"."t_upfile"."n_order" IS '显示顺序';
-- Create "t_user_login" table
CREATE TABLE "db_wenku"."t_user_login" (
    "c_id" character varying(50) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_corpname" character varying(300) NULL,
    "c_userid" character varying(300) NULL,
    "dt_time" timestamp NULL,
    "c_ny" character varying(100) NULL,
    "c_nyr" character varying(100) NULL,
    "c_ip" character varying(300) NULL,
    "n_status" integer NULL DEFAULT 1,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_user_login"
COMMENT ON TABLE "db_wenku"."t_user_login" IS '用户登录记录表';
-- Set comment to column: "c_id" on table: "t_user_login"
COMMENT ON COLUMN "db_wenku"."t_user_login"."c_id" IS 'id';
-- Set comment to column: "c_corpid" on table: "t_user_login"
COMMENT ON COLUMN "db_wenku"."t_user_login"."c_corpid" IS '单位代码';
-- Set comment to column: "c_userid" on table: "t_user_login"
COMMENT ON COLUMN "db_wenku"."t_user_login"."c_userid" IS '用户登录标识';
-- Set comment to column: "dt_time" on table: "t_user_login"
COMMENT ON COLUMN "db_wenku"."t_user_login"."dt_time" IS '登录时间';
-- Create "t_usertemplate" table
CREATE TABLE "db_wenku"."t_usertemplate" (
    "c_id" character varying(32) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_loginid" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "c_filename" character varying(300) NULL,
    "n_status" integer NULL,
    "dt_time" timestamp NULL,
    "n_order" integer NULL,
    "c_filepath" character varying(900) NULL,
    "c_jsonpath" character varying(900) NULL,
    "c_imagepath" character varying(900) NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_usertemplate"
COMMENT ON TABLE "db_wenku"."t_usertemplate" IS '用户自定义模版表';
-- Set comment to column: "c_id" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_id" IS '主键';
-- Set comment to column: "c_corpid" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_corpid" IS '用户所在单位代码';
-- Set comment to column: "c_loginid" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_loginid" IS '用户登录标识';
-- Set comment to column: "c_name" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_name" IS '模板名称';
-- Set comment to column: "c_filename" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_filename" IS '模板文件名';
-- Set comment to column: "n_status" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."n_status" IS '状态';
-- Set comment to column: "dt_time" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."dt_time" IS '更新时间';
-- Set comment to column: "n_order" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."n_order" IS '显示顺序';
-- Set comment to column: "c_filepath" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_filepath" IS '模版文件路径';
-- Set comment to column: "c_jsonpath" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_jsonpath" IS 'json文件路径';
-- Set comment to column: "c_imagepath" on table: "t_usertemplate"
COMMENT ON COLUMN "db_wenku"."t_usertemplate"."c_imagepath" IS '缩略图文件路径';
-- Create "t_version" table
CREATE TABLE "db_wenku"."t_version" (
    "c_id" character varying(50) NOT NULL,
    "c_modulename" character varying(200) NULL,
    "c_version" character varying(200) NULL,
    "dt_updatetime" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Set comment to table: "t_version"
COMMENT ON TABLE "db_wenku"."t_version" IS '版本信息表';
-- Set comment to column: "c_id" on table: "t_version"
COMMENT ON COLUMN "db_wenku"."t_version"."c_id" IS 'id';
-- Set comment to column: "c_modulename" on table: "t_version"
COMMENT ON COLUMN "db_wenku"."t_version"."c_modulename" IS '模块名称';
-- Set comment to column: "c_version" on table: "t_version"
COMMENT ON COLUMN "db_wenku"."t_version"."c_version" IS '版本号';
-- Set comment to column: "dt_updatetime" on table: "t_version"
COMMENT ON COLUMN "db_wenku"."t_version"."dt_updatetime" IS '更新时间';
-- Create "t_wdzs_wj" table
CREATE TABLE "db_wenku"."t_wdzs_wj" (
    "c_id" character varying(32) NOT NULL,
    "c_corpid" character varying(300) NULL,
    "c_loginid" character varying(300) NULL,
    "c_username" character varying(300) NULL,
    "c_bk" character varying(50) NULL,
    "c_zt" character varying(300) NULL,
    "n_xysh" integer NULL,
    "n_jzjkj" integer NULL,
    "c_wjm" character varying(600) NULL,
    "c_wjlj" character varying(900) NULL,
    "c_wjms" character varying(900) NULL,
    "n_shzt" integer NULL,
    "dt_shsj" timestamp NULL,
    "c_shr" character varying(300) NULL,
    "c_shbtgyy" character varying(900) NULL,
    "n_drzt" integer NULL,
    "dt_drsj" timestamp NULL,
    "c_drjg" character varying(300) NULL,
    "dt_cjsj" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_wdzsk_item" table
CREATE TABLE "db_wenku"."t_wdzsk_item" (
    "c_id" character varying(50) NOT NULL,
    "c_knowledge_base_id" character varying(50) NOT NULL,
    "c_parent_id" character varying(50) NULL,
    "c_type" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_path" text NULL,
    "n_level" integer NULL DEFAULT 1,
    "n_file_count" integer NULL DEFAULT 0,
    "c_file_path" character varying(500) NULL,
    "c_file_size" bigint NULL,
    "n_word_count" integer NULL DEFAULT 0,
    "c_es_index_id" character varying(100) NULL,
    "n_favorite" integer NULL DEFAULT 0,
    "c_minio_url" character varying(500) NULL,
    "c_create_user_id" character varying(50) NULL,
    "c_update_user_id" character varying(50) NULL,
    "dt_create_time" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    "dt_update_time" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    "n_status" integer NULL DEFAULT 1,
    "c_remark" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "idx_item_create_time" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_create_time" ON "db_wenku"."t_wdzsk_item" ("dt_create_time");
-- Create index "idx_item_es_index_id" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_es_index_id" ON "db_wenku"."t_wdzsk_item" ("c_es_index_id");
-- Create index "idx_item_favorite" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_favorite" ON "db_wenku"."t_wdzsk_item" ("n_favorite");
-- Create index "idx_item_kb_id" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_kb_id" ON "db_wenku"."t_wdzsk_item" ("c_knowledge_base_id");
-- Create index "idx_item_kb_parent" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_kb_parent" ON "db_wenku"."t_wdzsk_item" ("c_knowledge_base_id", "c_parent_id");
-- Create index "idx_item_kb_type" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_kb_type" ON "db_wenku"."t_wdzsk_item" ("c_knowledge_base_id", "c_type");
-- Create index "idx_item_name" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_name" ON "db_wenku"."t_wdzsk_item" ("c_name");
-- Create index "idx_item_parent_id" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_parent_id" ON "db_wenku"."t_wdzsk_item" ("c_parent_id");
-- Create index "idx_item_parent_type" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_parent_type" ON "db_wenku"."t_wdzsk_item" ("c_parent_id", "c_type");
-- Create index "idx_item_status" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_status" ON "db_wenku"."t_wdzsk_item" ("n_status");
-- Create index "idx_item_type" to table: "t_wdzsk_item"
CREATE INDEX "idx_item_type" ON "db_wenku"."t_wdzsk_item" ("c_type");
-- Set comment to table: "t_wdzsk_item"
COMMENT ON TABLE "db_wenku"."t_wdzsk_item" IS '文件和文件夹统一表';
-- Set comment to column: "c_id" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_id" IS 'ID';
-- Set comment to column: "c_knowledge_base_id" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_knowledge_base_id" IS '所属知识库ID';
-- Set comment to column: "c_parent_id" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_parent_id" IS '父文件夹ID，NULL表示在知识库根目录';
-- Set comment to column: "c_type" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_type" IS '类型：folder-文件夹，doc/pdf/xls/ppt等-文件类型';
-- Set comment to column: "c_name" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_name" IS '名称';
-- Set comment to column: "c_path" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_path" IS '路径（文件夹路径）';
-- Set comment to column: "n_level" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."n_level" IS '层级，1为第一层';
-- Set comment to column: "n_file_count" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."n_file_count" IS '文件数量';
-- Set comment to column: "c_file_path" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_file_path" IS '文件存储路径';
-- Set comment to column: "c_file_size" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_file_size" IS '文件大小（字节）';
-- Set comment to column: "n_word_count" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."n_word_count" IS '字数';
-- Set comment to column: "c_es_index_id" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_es_index_id" IS 'ES索引中的文档ID';
-- Set comment to column: "n_favorite" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."n_favorite" IS '是否收藏：1-是，0-否';
-- Set comment to column: "c_minio_url" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_minio_url" IS 'MinIO对象存储URL';
-- Set comment to column: "c_create_user_id" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_create_user_id" IS '创建人ID';
-- Set comment to column: "c_update_user_id" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_update_user_id" IS '更新人ID';
-- Set comment to column: "dt_create_time" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."dt_create_time" IS '创建时间';
-- Set comment to column: "dt_update_time" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."dt_update_time" IS '更新时间';
-- Set comment to column: "n_status" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."n_status" IS '状态：1-启用，0-禁用';
-- Set comment to column: "c_remark" on table: "t_wdzsk_item"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_item"."c_remark" IS '备注';
-- Create "t_wdzsk_knowledge_base" table
CREATE TABLE "db_wenku"."t_wdzsk_knowledge_base" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NOT NULL,
    "c_description" text NULL,
    "n_file_count" integer NULL DEFAULT 0,
    "c_create_user_id" character varying(50) NULL,
    "c_update_user_id" character varying(50) NULL,
    "dt_create_time" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    "dt_update_time" timestamp NULL DEFAULT CURRENT_TIMESTAMP,
    "n_status" integer NULL DEFAULT 1,
    "c_remark" text NULL,
    PRIMARY KEY ("c_id")
);
-- Create index "idx_kb_create_time" to table: "t_wdzsk_knowledge_base"
CREATE INDEX "idx_kb_create_time" ON "db_wenku"."t_wdzsk_knowledge_base" ("dt_create_time");
-- Create index "idx_kb_create_user" to table: "t_wdzsk_knowledge_base"
CREATE INDEX "idx_kb_create_user" ON "db_wenku"."t_wdzsk_knowledge_base" ("c_create_user_id");
-- Create index "idx_kb_name" to table: "t_wdzsk_knowledge_base"
CREATE INDEX "idx_kb_name" ON "db_wenku"."t_wdzsk_knowledge_base" ("c_name");
-- Create index "idx_kb_status" to table: "t_wdzsk_knowledge_base"
CREATE INDEX "idx_kb_status" ON "db_wenku"."t_wdzsk_knowledge_base" ("n_status");
-- Set comment to table: "t_wdzsk_knowledge_base"
COMMENT ON TABLE "db_wenku"."t_wdzsk_knowledge_base" IS '知识库表';
-- Set comment to column: "c_id" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."c_id" IS '知识库ID';
-- Set comment to column: "c_name" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."c_name" IS '知识库名称';
-- Set comment to column: "c_description" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."c_description" IS '知识库描述';
-- Set comment to column: "n_file_count" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."n_file_count" IS '文件数量';
-- Set comment to column: "c_create_user_id" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."c_create_user_id" IS '创建人ID';
-- Set comment to column: "dt_create_time" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."dt_create_time" IS '创建时间';
-- Set comment to column: "dt_update_time" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."dt_update_time" IS '更新时间';
-- Set comment to column: "n_status" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."n_status" IS '状态：1-启用，0-禁用';
-- Set comment to column: "c_remark" on table: "t_wdzsk_knowledge_base"
COMMENT ON COLUMN "db_wenku"."t_wdzsk_knowledge_base"."c_remark" IS '备注';
-- Create "t_xmconfig" table
CREATE TABLE "db_wenku"."t_xmconfig" (
    "c_id" character varying(50) NOT NULL,
    "c_name" character varying(300) NULL,
    "c_key" character varying(100) NULL,
    "c_value" character varying(300) NULL,
    "n_order" integer NULL,
    "dt_createtime" timestamp NULL,
    "dt_updatetime" timestamp NULL,
    PRIMARY KEY ("c_id")
);
-- Create "t_xzsysconfig" table
CREATE TABLE "db_wenku"."t_xzsysconfig" (
    "c_id" character varying(50) NOT NULL,
    "c_group" character varying(300) NULL,
    "c_name" character varying(300) NULL,
    "c_key" character varying(100) NULL,
    "c_value" character varying(300) NULL,
    "n_order" integer NULL,
    "dt_create_time" timestamp NULL,
    "dt_update_time" timestamp NULL,
    PRIMARY KEY ("c_id")
);
