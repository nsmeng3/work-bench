table "jpaeventpublication" {
  schema = schema.db_wenku
  column "id" {
    null = false
    type = uuid
  }
  column "completiondate" {
    null = true
    type = timestamp
  }
  column "eventtype" {
    null = true
    type = character_varying(255)
  }
  column "listenerid" {
    null = true
    type = character_varying(255)
  }
  column "publicationdate" {
    null = true
    type = timestamp
  }
  column "serializedevent" {
    null = true
    type = character_varying(255)
  }
  primary_key {
    columns = [column.id]
  }
}
table "t_advertisement" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_key" {
    null = true
    type = character_varying(300)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "c_value" {
    null = true
    type = character_varying(300)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_ai_feedback" {
  schema  = schema.db_wenku
  comment = "AI数据表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_userid" {
    null = true
    type = character_varying(300)
  }
  column "c_groupid" {
    null    = false
    type    = character_varying(50)
    comment = "groupId，问答里面可能有记录groupId相同，起草和改写没有"
  }
  column "c_type" {
    null    = false
    type    = character_varying(100)
    comment = "类型，起草|改写|问答"
  }
  column "c_input" {
    null    = true
    type    = text
    comment = "用户输入，改写可能为空"
  }
  column "c_gxlx" {
    null    = true
    type    = character_varying(100)
    comment = "改写类型，精简|段落扩写|续写|总结|大纲扩写，只有改写有值，起草和问答为空"
  }
  column "c_xznr" {
    null    = true
    type    = text
    comment = "改写时选中的内容，只有改写有值，起草和问答为空"
  }
  column "c_prompt" {
    null    = false
    type    = text
    comment = "问题"
  }
  column "c_answer" {
    null    = false
    type    = text
    comment = "答案"
  }
  column "n_jgzs" {
    null = true
    type = integer
  }
  column "n_like" {
    null    = false
    type    = integer
    comment = "是否点赞，0：未评价，1：点赞，-1：点差"
  }
  column "c_lastmodify" {
    null    = false
    type    = character_varying(100)
    comment = "最后修改时间"
  }
  column "c_ip" {
    null    = true
    type    = character_varying(300)
    comment = "请求ip"
  }
  column "c_nyr" {
    null    = true
    type    = character_varying(100)
    comment = "年月日，格式为：yyyy年MM月dd日"
  }
  column "c_ny" {
    null    = true
    type    = character_varying(100)
    comment = "年月，格式为：yyyy年MM月"
  }
  column "dt_time" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_t_ai_feedback_like" {
    columns = [column.n_like]
  }
  index "i_t_ai_feedback_type" {
    columns = [column.c_type]
  }
}
table "t_analyze" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_type" {
    null    = true
    type    = character_varying(300)
    comment = "文章类型"
  }
  column "c_role" {
    null    = true
    type    = character_varying(300)
    comment = "角色类型"
  }
  column "c_content" {
    null    = true
    type    = text
    comment = "提示词内容"
  }
  column "n_retain" {
    null    = true
    type    = integer
    default = 0
    comment = "是否保留"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_app" {
  schema  = schema.db_wenku
  comment = "应用表"
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_app_id" {
    null    = true
    type    = character_varying(100)
    comment = "应用appId"
  }
  column "c_name" {
    null    = true
    type    = character_varying(300)
    comment = "应用名称"
  }
  column "c_api_key" {
    null    = true
    type    = character_varying(300)
    comment = "应用密钥"
  }
  column "c_mode" {
    null    = true
    type    = character_varying(32)
    comment = "应用类型"
  }
  column "c_description" {
    null    = true
    type    = character_varying(1000)
    comment = "应用描述"
  }
  column "c_icon" {
    null = true
    type = text
  }
  column "c_workspace_name" {
    null    = true
    type    = character_varying(300)
    comment = "应用所属工作空间名称"
  }
  column "dt_create_time" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "c_enable" {
    null    = false
    type    = character_varying(50)
    default = "1"
    comment = "是否启用"
  }
  column "n_sort" {
    null    = false
    type    = integer
    default = 1
    comment = "应用顺序"
  }
  column "c_bg" {
    null = true
    type = character_varying(100)
  }
  column "c_emoji" {
    null = true
    type = character_varying(10)
  }
  column "c_av_flag" {
    null = true
    type = character_varying(2)
  }
  primary_key "pk_t_app" {
    columns = [column.c_id]
  }
}
table "t_app_chunk" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_app_id" {
    null = true
    type = character_varying(50)
  }
  column "c_conversation_id" {
    null = true
    type = character_varying(300)
  }
  column "c_user_id" {
    null = true
    type = character_varying(50)
  }
  column "c_message_id" {
    null = true
    type = character_varying(300)
  }
  column "c_chunks" {
    null = true
    type = text
  }
  column "dt_create_time" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_aty_cfgid" {
  schema = schema.db_wenku
  column "c_key" {
    null = false
    type = character_varying(100)
  }
  column "n_value" {
    null    = false
    type    = numeric(17,2)
    default = 0
  }
  column "n_fldlength" {
    null    = false
    type    = numeric(3)
    default = 16
  }
  primary_key "pk_aty_cfgid" {
    columns = [column.c_key]
  }
}
table "t_aty_classcode" {
  schema = schema.db_wenku
  column "c_type" {
    null = false
    type = character_varying(100)
  }
  column "c_code" {
    null = false
    type = character_varying(100)
  }
  column "c_pid" {
    null = true
    type = character_varying(300)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "n_valid" {
    null = false
    type = integer
  }
  column "c_dmjp" {
    null = true
    type = character_varying(300)
  }
  column "n_order" {
    null = false
    type = integer
  }
  column "c_ext" {
    null = true
    type = text
  }
  primary_key "pk_t_aty_classcode" {
    columns = [column.c_type, column.c_code]
  }
}
table "t_aty_code" {
  schema = schema.db_wenku
  column "c_pid" {
    null    = false
    type    = character_varying(50)
    default = "0"
  }
  column "c_code" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "n_kwh" {
    null    = false
    type    = numeric(3)
    default = 2
  }
  column "c_levelinfo" {
    null = true
    type = character_varying(300)
  }
  column "n_valid" {
    null    = false
    type    = numeric(3)
    default = 1
  }
  column "n_order" {
    null    = false
    type    = smallint
    default = 1
  }
  column "c_dmjp" {
    null = true
    type = character_varying(300)
  }
  primary_key "pk_aty_code" {
    columns = [column.c_pid, column.c_code]
  }
}
table "t_aty_codetype" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "n_valid" {
    null    = false
    type    = numeric(3)
    default = 1
  }
  column "n_sfkwh" {
    null    = false
    type    = numeric(3)
    default = 2
  }
  primary_key "pk_aty_codetype" {
    columns = [column.c_id]
  }
}
table "t_aty_config" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_key" {
    null = false
    type = character_varying(100)
  }
  column "c_value" {
    null = true
    type = character_varying(300)
  }
  column "c_descript" {
    null = true
    type = text
  }
  primary_key "pk_t_aty_config" {
    columns = [column.c_id]
  }
}
table "t_aty_corp" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "c_pid" {
    null = true
    type = character_varying(300)
  }
  column "n_level" {
    null    = false
    type    = numeric(3)
    default = 4
  }
  column "c_gbm" {
    null = true
    type = character_varying(300)
  }
  column "c_alias" {
    null = true
    type = character_varying(300)
  }
  column "n_valid" {
    null    = false
    type    = numeric(3)
    default = 1
  }
  column "n_order" {
    null    = false
    type    = smallint
    default = 1
  }
  column "c_ext" {
    null = true
    type = text
  }
  primary_key "pk_aty_corp" {
    columns = [column.c_id]
  }
}
table "t_aty_dept" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "c_pid" {
    null = true
    type = character_varying(300)
  }
  column "c_corp" {
    null = true
    type = character_varying(300)
  }
  column "c_alias" {
    null = true
    type = character_varying(300)
  }
  column "n_valid" {
    null    = false
    type    = numeric(3)
    default = 1
  }
  column "n_order" {
    null    = false
    type    = smallint
    default = 1
  }
  column "c_ext" {
    null = true
    type = text
  }
  primary_key "pk_aty_dept" {
    columns = [column.c_id]
  }
}
table "t_aty_dictcustom" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_groupid" {
    null = false
    type = character_varying(300)
  }
  column "c_tablekey" {
    null = true
    type = character_varying(300)
  }
  column "c_tablename" {
    null = true
    type = character_varying(300)
  }
  column "c_fieldname" {
    null = true
    type = character_varying(300)
  }
  column "c_propname" {
    null = true
    type = character_varying(300)
  }
  column "c_value" {
    null = true
    type = character_varying(300)
  }
  primary_key "pk_aty_dictcustom" {
    columns = [column.c_id]
  }
}
table "t_aty_log" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_type" {
    null = false
    type = character_varying(50)
  }
  column "c_module" {
    null = false
    type = character_varying(300)
  }
  column "c_function" {
    null = false
    type = character_varying(300)
  }
  column "c_content" {
    null = false
    type = text
  }
  column "dt_time" {
    null = false
    type = timestamp
  }
  column "c_result" {
    null = false
    type = character_varying(100)
  }
  column "c_host" {
    null = false
    type = character_varying(100)
  }
  column "c_userid" {
    null = true
    type = character_varying(32)
  }
  column "c_username" {
    null = true
    type = character_varying(300)
  }
  column "c_loginid" {
    null = true
    type = character_varying(300)
  }
  column "c_corpid" {
    null = true
    type = character_varying(32)
  }
  column "c_corpname" {
    null = true
    type = character_varying(300)
  }
  column "c_deptid" {
    null = true
    type = character_varying(32)
  }
  column "c_deptname" {
    null = true
    type = character_varying(300)
  }
  column "c_md5" {
    null = false
    type = character_varying(32)
  }
  column "c_ext" {
    null = true
    type = text
  }
  primary_key "pk_t_aty_log" {
    columns = [column.c_id]
  }
}
table "t_aty_log_archive" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_type" {
    null = false
    type = character_varying(50)
  }
  column "c_month" {
    null = false
    type = character_varying(50)
  }
  column "c_descript" {
    null = true
    type = text
  }
  column "n_count" {
    null = true
    type = integer
  }
  column "dt_time" {
    null = false
    type = date
  }
  primary_key "pk_t_aty_log_archive" {
    columns = [column.c_id]
  }
}
table "t_aty_log_back" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_type" {
    null = false
    type = character_varying(50)
  }
  column "c_module" {
    null = false
    type = character_varying(300)
  }
  column "c_function" {
    null = false
    type = character_varying(300)
  }
  column "c_content" {
    null = false
    type = text
  }
  column "dt_time" {
    null = false
    type = date
  }
  column "c_result" {
    null = false
    type = character_varying(100)
  }
  column "c_host" {
    null = false
    type = character_varying(100)
  }
  column "c_userid" {
    null = true
    type = character_varying(32)
  }
  column "c_username" {
    null = true
    type = character_varying(300)
  }
  column "c_loginid" {
    null = true
    type = character_varying(300)
  }
  column "c_corpid" {
    null = true
    type = character_varying(32)
  }
  column "c_corpname" {
    null = true
    type = character_varying(300)
  }
  column "c_deptid" {
    null = true
    type = character_varying(32)
  }
  column "c_deptname" {
    null = true
    type = character_varying(300)
  }
  column "c_md5" {
    null = false
    type = character_varying(32)
  }
  column "c_ext" {
    null = true
    type = text
  }
  primary_key "pk_t_aty_log_back" {
    columns = [column.c_id]
  }
}
table "t_aty_planexeclog" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_planid" {
    null = true
    type = character_varying(32)
  }
  column "n_logtype" {
    null = true
    type = numeric(3)
  }
  column "d_time" {
    null = true
    type = timestamp
  }
  column "c_loginfo" {
    null = true
    type = character_varying(300)
  }
  column "c_logexception" {
    null = true
    type = text
  }
  primary_key "pk_aty_planexeclog" {
    columns = [column.c_id]
  }
}
table "t_aty_right" {
  schema = schema.db_wenku
  column "c_rightkey" {
    null = false
    type = character_varying(150)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "c_descript" {
    null = true
    type = character_varying(300)
  }
  column "n_order" {
    null = true
    type = integer
  }
  primary_key "pk_aty_right" {
    columns = [column.c_rightkey]
  }
}
table "t_aty_role" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "c_descript" {
    null = true
    type = character_varying(300)
  }
  column "n_xtgy" {
    null    = true
    type    = numeric(3)
    default = 2
  }
  column "n_valid" {
    null    = true
    type    = numeric(3)
    default = 1
  }
  column "n_order" {
    null    = true
    type    = smallint
    default = 1
  }
  primary_key "pk_aty_role" {
    columns = [column.c_id]
  }
}
table "t_aty_role_right" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_roleid" {
    null = false
    type = character_varying(300)
  }
  column "c_rightkey" {
    null = false
    type = character_varying(300)
  }
  primary_key "pk_aty_role_right" {
    columns = [column.c_id]
  }
}
table "t_aty_user" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_loginid" {
    null = false
    type = character_varying(300)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "c_password" {
    null    = false
    type    = character_varying(100)
    default = "D41D8CD98F00B204E9800998ECF8427E"
  }
  column "c_mail" {
    null = true
    type = character_varying(300)
  }
  column "c_ip" {
    null = true
    type = character_varying(300)
  }
  column "c_xmjp" {
    null = true
    type = character_varying(300)
  }
  column "c_corp" {
    null = true
    type = character_varying(300)
  }
  column "c_dept" {
    null = true
    type = character_varying(300)
  }
  column "n_valid" {
    null    = false
    type    = numeric(3)
    default = 1
  }
  column "n_order" {
    null    = false
    type    = smallint
    default = 1
  }
  column "n_retry" {
    null = true
    type = integer
  }
  column "dt_password_update" {
    null = true
    type = date
  }
  column "c_ext" {
    null = true
    type = text
  }
  primary_key "pk_aty_user" {
    columns = [column.c_id]
  }
}
table "t_aty_user_right" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_userid" {
    null = false
    type = character_varying(300)
  }
  column "n_type" {
    null    = false
    type    = numeric(3)
    default = 1
  }
  column "c_roleid" {
    null = true
    type = character_varying(300)
  }
  column "c_rightkey" {
    null = true
    type = character_varying(300)
  }
  primary_key "pk_aty_user_right" {
    columns = [column.c_id]
  }
}
table "t_aty_writ" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_tid" {
    null = false
    type = character_varying(32)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "n_version" {
    null = true
    type = numeric(3)
  }
  column "i_writ" {
    null = true
    type = bytea
  }
  column "c_writhtml" {
    null = true
    type = text
  }
  column "d_updatetime" {
    null = true
    type = timestamp
  }
  column "c_ryid" {
    null = true
    type = character_varying(300)
  }
  primary_key "pk_aty_writ" {
    columns = [column.c_id]
  }
}
table "t_aty_writlog" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_writid" {
    null = false
    type = character_varying(32)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "n_version" {
    null = true
    type = numeric(3)
  }
  column "i_writ" {
    null = true
    type = bytea
  }
  column "c_writhtml" {
    null = true
    type = bytea
  }
  column "d_updatetime" {
    null = true
    type = timestamp
  }
  column "c_ryid" {
    null = true
    type = character_varying(300)
  }
  primary_key "pk_aty_writlog" {
    columns = [column.c_id]
  }
}
table "t_audio" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_user_id" {
    null = true
    type = character_varying(300)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "n_state" {
    null = true
    type = integer
  }
  column "c_duration" {
    null = true
    type = character_varying(300)
  }
  column "dt_create_time" {
    null = true
    type = timestamp
  }
  column "c_ext" {
    null = true
    type = text
  }
  column "c_notes" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_audio_name" {
    columns = [column.c_name]
  }
  index "i_audio_user_id" {
    columns = [column.c_user_id]
  }
}
table "t_audio_duration_stats" {
  schema  = schema.db_wenku
  comment = "用户录音转写总时长统计表"
  column "c_user_id" {
    null    = false
    type    = character_varying(300)
    comment = "用户ID"
  }
  column "n_total_duration_seconds" {
    null    = true
    type    = bigint
    default = 0
    comment = "总录音时长（秒）"
  }
  primary_key {
    columns = [column.c_user_id]
  }
  index "i_audio_duration_stats_user_id" {
    columns = [column.c_user_id]
  }
}
table "t_audio_sep" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_audio_id" {
    null = true
    type = character_varying(50)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "c_begin_time" {
    null = true
    type = character_varying(300)
  }
  column "c_end_time" {
    null = true
    type = character_varying(300)
  }
  column "c_content" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_audio_sep_audio_id" {
    columns = [column.c_audio_id]
  }
}
table "t_audio_summary" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_audio_id" {
    null = true
    type = character_varying(50)
  }
  column "c_title" {
    null = true
    type = character_varying(300)
  }
  column "c_content" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_audio_summary_audio_id" {
    columns = [column.c_audio_id]
  }
}
table "t_audioconfig" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "c_key" {
    null = true
    type = character_varying(100)
  }
  column "c_value" {
    null = true
    type = character_varying(300)
  }
  column "n_order" {
    null = true
    type = integer
  }
  column "dt_create_time" {
    null = true
    type = timestamp
  }
  column "dt_update_time" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_audiot_hotword" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_user_id" {
    null = true
    type = character_varying(300)
  }
  column "c_hotword_id" {
    null = true
    type = character_varying(300)
  }
  column "c_hotword" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_audio_hotword_hotword_id" {
    columns = [column.c_hotword_id]
  }
  index "i_audio_hotword_user_id" {
    columns = [column.c_user_id]
  }
}
table "t_bim_org" {
  schema  = schema.db_wenku
  comment = "BIM 组织机构映射表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "主键"
  }
  column "c_bim_org_id" {
    null    = true
    type    = character_varying(300)
    comment = "平台组织机构主键"
  }
  column "c_local_org_id" {
    null    = true
    type    = character_varying(300)
    comment = "Artery 内部组织机构 ID"
  }
  column "c_org_name" {
    null    = true
    type    = character_varying(300)
    comment = "组织机构名称"
  }
  column "c_par_bim_org_id" {
    null    = true
    type    = character_varying(300)
    comment = "平台上级组织机构主键"
  }
  column "n_enable" {
    null    = true
    type    = smallint
    default = 1
    comment = "是否启用：1-启用，0-禁用"
  }
  column "dt_cjsj" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_t_bim_org_bim_id" {
    columns = [column.c_bim_org_id]
  }
  index "i_t_bim_org_local_id" {
    columns = [column.c_local_org_id]
  }
}
table "t_bim_user" {
  schema  = schema.db_wenku
  comment = "BIM 用户映射表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "主键"
  }
  column "c_bim_uid" {
    null    = true
    type    = character_varying(300)
    comment = "平台账号主键"
  }
  column "c_local_user_id" {
    null    = true
    type    = character_varying(300)
    comment = "Artery 内部用户 ID"
  }
  column "c_login_name" {
    null    = true
    type    = character_varying(300)
    comment = "登录名"
  }
  column "c_full_name" {
    null    = true
    type    = character_varying(300)
    comment = "用户姓名"
  }
  column "c_bim_org_id" {
    null    = true
    type    = character_varying(300)
    comment = "平台组织机构主键"
  }
  column "c_personal_confidentiality_level" {
    null    = true
    type    = character_varying(64)
    comment = "人员涉密等级"
  }
  column "n_enable" {
    null    = true
    type    = smallint
    default = 1
    comment = "是否启用：1-启用，0-禁用"
  }
  column "dt_cjsj" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_t_bim_user_bim_uid" {
    columns = [column.c_bim_uid]
  }
  index "i_t_bim_user_local_id" {
    columns = [column.c_local_user_id]
  }
  index "i_t_bim_user_login_name" {
    columns = [column.c_login_name]
  }
  index "i_t_bim_user_pcl" {
    columns = [column.c_personal_confidentiality_level]
  }
}
table "t_chat_history" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = bigserial
  }
  column "c_session_id" {
    null = false
    type = character_varying(50)
  }
  column "c_user_id" {
    null = false
    type = character_varying(50)
  }
  column "c_message_type" {
    null = false
    type = character_varying(20)
  }
  column "c_content" {
    null = false
    type = text
  }
  column "c_created_at" {
    null = false
    type = bigint
  }
  column "c_updated_at" {
    null = true
    type = timestamp
  }
  column "c_message_order" {
    null = true
    type = integer
  }
  column "c_title" {
    null = true
    type = text
  }
  column "c_model_used" {
    null = true
    type = character_varying(100)
  }
  column "c_tags" {
    null = true
    type = character_varying(255)
  }
  column "c_is_deleted" {
    null    = true
    type    = boolean
    default = false
  }
  column "c_is_pinned" {
    null    = true
    type    = boolean
    default = false
  }
  column "c_conversation_id" {
    null = true
    type = character_varying(50)
  }
  column "c_main_content" {
    null = true
    type = text
  }
  column "c_answer_content" {
    null = true
    type = text
  }
  column "c_message_id" {
    null = true
    type = character_varying(300)
  }
  primary_key "chat_history_pkey" {
    columns = [column.c_id]
  }
  index "idx_chat_history_conversation_id" {
    columns = [column.c_conversation_id]
  }
  check "chat_history_t_message_type_check" {
    expr = "((c_message_type)::text = ANY ((ARRAY['user'::character varying, 'ai'::character varying])::text[]))"
  }
}
table "t_client" {
  schema  = schema.db_wenku
  comment = "客户端记录表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_ip" {
    null    = true
    type    = character_varying(300)
    comment = "IP"
  }
  column "dt_cjsj" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "dt_zhgxsj" {
    null    = true
    type    = timestamp
    comment = "最后更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_client_user" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_user" {
    null = true
    type = character_varying(100)
  }
  column "dt_cjsj" {
    null = true
    type = timestamp
  }
  column "dt_zhgxsj" {
    null = true
    type = timestamp
  }
  primary_key "t_client_user_pk" {
    columns = [column.c_id]
  }
}
table "t_clientui" {
  schema  = schema.db_wenku
  comment = "客户端显示配置表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_name" {
    null    = true
    type    = character_varying(300)
    comment = "显示项名称"
  }
  column "c_group" {
    null    = true
    type    = character_varying(100)
    comment = "显示项关键字"
  }
  column "c_normalshowtype" {
    null    = true
    type    = character_varying(10)
    comment = "office中显示类型"
  }
  column "c_embedshowtype" {
    null    = true
    type    = character_varying(10)
    comment = "页面中显示类型"
  }
  column "c_normalmessage" {
    null    = true
    type    = character_varying(300)
    comment = "office中提示信息"
  }
  column "c_embedmessage" {
    null    = true
    type    = character_varying(300)
    comment = "页面中提示信息"
  }
  column "dt_createtime" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "n_order" {
    null    = true
    type    = integer
    comment = "显示顺序"
  }
  column "dt_updatetime" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_config" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_key" {
    null    = false
    type    = character_varying(100)
    comment = "配置项的关键字"
  }
  column "c_value" {
    null    = true
    type    = character_varying(300)
    comment = "配置项的值"
  }
  column "c_descript" {
    null    = true
    type    = text
    comment = "配置项的描述"
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_corpauth" {
  schema  = schema.db_wenku
  comment = "单位授权信息表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_corpname" {
    null = true
    type = character_varying(300)
  }
  column "dt_expiredtime" {
    null    = true
    type    = timestamp
    comment = "到期时间"
  }
  column "n_maxnumberofuser" {
    null    = true
    type    = integer
    comment = "最大用户数"
  }
  column "c_admin" {
    null    = true
    type    = character_varying(300)
    comment = "管理员账号"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_sfcjzh" {
    null = true
    type = character_varying(50)
  }
  column "n_cjzhgs" {
    null = true
    type = integer
  }
  column "c_zhqz" {
    null = true
    type = character_varying(300)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_corpconfig" {
  schema  = schema.db_wenku
  comment = "单位配置表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_key" {
    null    = true
    type    = character_varying(100)
    comment = "配置项关键字"
  }
  column "c_value" {
    null    = true
    type    = character_varying(300)
    comment = "显示项关键字"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_corpuser" {
  schema  = schema.db_wenku
  comment = "单位用户登录记录表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_userid" {
    null    = true
    type    = character_varying(300)
    comment = "用户登录标识"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_ny" {
    null = true
    type = character_varying(100)
  }
  column "c_nyr" {
    null = true
    type = character_varying(100)
  }
  column "c_corpname" {
    null = true
    type = character_varying(300)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_credential_keys" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_key" {
    null = true
    type = character_varying(200)
  }
  column "c_login_id" {
    null = true
    type = character_varying(50)
  }
  column "c_show_name" {
    null = true
    type = character_varying(50)
  }
  column "n_timestamp" {
    null = true
    type = bigint
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_custom_index_data" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_es_id" {
    null = true
    type = character_varying(300)
  }
  column "c_syn_id" {
    null = true
    type = character_varying(300)
  }
  column "c_index" {
    null = true
    type = character_varying(50)
  }
  column "c_dataset_id" {
    null = true
    type = character_varying(300)
  }
  column "n_type" {
    null = true
    type = integer
  }
  column "dt_syn_time" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
  index "t_custom_index_data_c_es_id_index" {
    columns = [column.c_es_id]
  }
  index "t_custom_index_data_c_index_index" {
    columns = [column.c_index]
  }
}
table "t_data_import_module" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_module" {
    null = true
    type = character_varying(50)
  }
  column "c_name" {
    null = true
    type = character_varying(255)
  }
  column "c_user" {
    null = true
    type = character_varying
  }
  column "c_right" {
    null = true
    type = text
  }
  column "dt_time" {
    null = true
    type = timestamp
  }
  column "c_type" {
    null = true
    type = character_varying(30)
  }
  column "login_type" {
    null    = true
    type    = integer
    default = 0
  }
  column "c_topic" {
    null    = true
    type    = character_varying(32)
    comment = "主题"
  }
  column "c_section" {
    null    = true
    type    = character_varying(32)
    comment = "板块"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_data_module" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_name" {
    null = true
    type = character_varying(100)
  }
  column "c_index" {
    null = true
    type = character_varying(100)
  }
  column "c_type" {
    null = true
    type = character_varying(50)
  }
  column "n_order" {
    null = false
    type = integer
  }
  column "c_knowledge_id" {
    null = true
    type = character_varying(300)
  }
  column "c_dept" {
    null = true
    type = text
  }
  column "c_dept_type" {
    null = true
    type = text
  }
  column "c_doc_type" {
    null    = true
    type    = character_varying(30)
    default = "normal"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_dify_zsk" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_zsk_id" {
    null    = true
    type    = character_varying(300)
    comment = "知识库的ID"
  }
  column "c_zsk_name" {
    null    = true
    type    = character_varying(300)
    comment = "知识库的名称"
  }
  column "c_zsk_mate" {
    null    = true
    type    = text
    comment = "知识库的元数据"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_ding_user" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_token" {
    null = true
    type = character_varying(50)
  }
  column "c_user_info" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_ding_user_token" {
    columns = [column.c_token]
  }
}
table "t_docsearch" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_ostype" {
    null    = false
    type    = character_varying(50)
    comment = "操作系统"
  }
  column "c_cputype" {
    null    = false
    type    = character_varying(50)
    comment = "CPU类型"
  }
  column "c_filesize" {
    null    = false
    type    = character_varying(300)
    comment = "文件大小"
  }
  column "c_objname" {
    null    = false
    type    = character_varying(900)
    comment = "文件名称"
  }
  column "c_exeaddr" {
    null    = false
    type    = text
    comment = "地址"
  }
  column "d_createtime" {
    null    = false
    type    = timestamp
    comment = "创建时间"
  }
  column "c_md5" {
    null    = true
    type    = character_varying(300)
    comment = "MD5值"
  }
  column "c_version" {
    null = true
    type = character_varying(30)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_dyn_wj" {
  schema  = schema.db_wenku
  comment = "我的知识-文件表"
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位id"
  }
  column "c_loginid" {
    null    = true
    type    = character_varying(300)
    comment = "登录标识"
  }
  column "c_username" {
    null    = true
    type    = character_varying(300)
    comment = "用户名"
  }
  column "c_index" {
    null    = true
    type    = character_varying(50)
    comment = "索引"
  }
  column "c_section" {
    null    = true
    type    = character_varying(50)
    comment = "板块"
  }
  column "c_topic" {
    null    = true
    type    = character_varying(300)
    comment = "主题"
  }
  column "n_xysh" {
    null    = true
    type    = integer
    comment = "是否需要审核"
  }
  column "n_jzjkj" {
    null    = true
    type    = integer
    comment = "是否仅自己可见"
  }
  column "c_wjm" {
    null    = true
    type    = character_varying(600)
    comment = "文件名"
  }
  column "c_wjlj" {
    null    = true
    type    = character_varying(900)
    comment = "文件路径"
  }
  column "c_wjms" {
    null    = true
    type    = character_varying(900)
    comment = "文件描述"
  }
  column "n_shzt" {
    null    = true
    type    = integer
    comment = "审核状态"
  }
  column "dt_shsj" {
    null    = true
    type    = timestamp
    comment = "审核时间"
  }
  column "c_shr" {
    null    = true
    type    = character_varying(300)
    comment = "审核人"
  }
  column "c_shbtgyy" {
    null    = true
    type    = character_varying(900)
    comment = "审核不通过原因"
  }
  column "n_drzt" {
    null    = true
    type    = integer
    comment = "导入状态"
  }
  column "dt_drsj" {
    null    = true
    type    = timestamp
    comment = "导入时间"
  }
  column "c_drjg" {
    null    = true
    type    = character_varying(300)
    comment = "导入结果"
  }
  column "dt_cjsj" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_favorite" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_docid" {
    null    = false
    type    = character_varying(300)
    comment = "素材id"
  }
  column "c_userid" {
    null    = false
    type    = character_varying(300)
    comment = "用户id"
  }
  column "c_time" {
    null    = false
    type    = character_varying(100)
    comment = "创建时间"
  }
  column "c_doctype" {
    null = true
    type = character_varying(50)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_client_version" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_version" {
    null = false
    type = character_varying(300)
  }
  column "c_ostype" {
    null = false
    type = character_varying(50)
  }
  column "c_cputype" {
    null = false
    type = character_varying(50)
  }
  column "c_frame" {
    null = true
    type = character_varying(50)
  }
  column "d_date" {
    null = false
    type = date
  }
  column "lc_updatelog" {
    null = true
    type = text
  }
  column "c_filesize" {
    null = false
    type = character_varying(300)
  }
  column "c_objname" {
    null = false
    type = character_varying(900)
  }
  column "c_exeaddr" {
    null = false
    type = text
  }
  column "d_createtime" {
    null = false
    type = timestamp
  }
  column "n_iscurrent" {
    null = false
    type = integer
  }
  column "c_md5" {
    null = true
    type = character_varying(300)
  }
  column "c_clientid" {
    null = true
    type = character_varying(300)
  }
  column "c_upgradeobjname" {
    null = true
    type = character_varying(900)
  }
  column "c_upgradeaddr" {
    null = true
    type = character_varying(900)
  }
  column "c_upgrademd5" {
    null = true
    type = character_varying(300)
  }
  column "n_isforceupdate" {
    null = true
    type = integer
  }
  column "c_cdnaddr" {
    null = true
    type = character_varying(900)
  }
  column "c_upgradecdnaddr" {
    null = true
    type = character_varying(900)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_corp_config" {
  schema  = schema.db_wenku
  comment = "单位信息配置表"
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_tenant_id" {
    null    = true
    type    = character_varying(300)
    comment = "租户编号"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_isuse" {
    null    = true
    type    = integer
    comment = "是否使用单位配置"
  }
  column "c_leader" {
    null    = true
    type    = text
    comment = "领导信息"
  }
  column "c_department" {
    null    = true
    type    = text
    comment = "部门信息"
  }
  column "c_extra" {
    null    = true
    type    = text
    comment = "额外信息"
  }
  column "c_prompt" {
    null    = true
    type    = text
    comment = "提示词模板"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_sc_zl" {
  schema  = schema.db_wenku
  comment = "公文编校收藏摘录"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_userid" {
    null    = false
    type    = character_varying(300)
    comment = "用户id"
  }
  column "c_source" {
    null    = false
    type    = character_varying(300)
    comment = "摘录来源"
  }
  column "c_time" {
    null    = false
    type    = character_varying(100)
    comment = "创建时间"
  }
  column "c_content" {
    null    = false
    type    = text
    comment = "摘录内容"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_template" {
  schema  = schema.db_wenku
  comment = "模版表"
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "主键"
  }
  column "c_corpid" {
    null = true
    type = character_varying(300)
  }
  column "c_name" {
    null    = true
    type    = character_varying(300)
    comment = "模板名称"
  }
  column "c_filename" {
    null    = true
    type    = character_varying(300)
    comment = "模板文件名"
  }
  column "c_type" {
    null    = true
    type    = character_varying(300)
    comment = "模版类型：default、import"
  }
  column "n_status" {
    null    = true
    type    = integer
    comment = "状态"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_groupid" {
    null    = true
    type    = character_varying(50)
    comment = "分组id"
  }
  column "n_order" {
    null    = true
    type    = integer
    comment = "显示顺序"
  }
  column "c_filepath" {
    null    = true
    type    = character_varying(900)
    comment = "模版文件路径"
  }
  column "c_xmlpath" {
    null    = true
    type    = character_varying(900)
    comment = "xml文件路径"
  }
  column "c_imagepath" {
    null    = true
    type    = character_varying(900)
    comment = "缩略图文件路径"
  }
  column "c_content_id" {
    null = true
    type = character_varying(300)
  }
  column "c_ban_id" {
    null = true
    type = character_varying(300)
  }
  column "c_jsonpath" {
    null = true
    type = character_varying(900)
  }
  column "c_attach_pos" {
    null = true
    type = integer
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_templategroup" {
  schema  = schema.db_wenku
  comment = "模版分组表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null = true
    type = character_varying(300)
  }
  column "c_name" {
    null    = true
    type    = character_varying(300)
    comment = "分组名称"
  }
  column "n_status" {
    null    = true
    type    = integer
    comment = "状态"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "n_order" {
    null = true
    type = integer
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_templatetime" {
  schema  = schema.db_wenku
  comment = "模版更新时间表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null = true
    type = character_varying(300)
  }
  column "dt_grouptime" {
    null    = true
    type    = timestamp
    comment = "分组最新更新时间"
  }
  column "dt_templatetime" {
    null    = true
    type    = timestamp
    comment = "模版最新更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_tj_errorword" {
  schema  = schema.db_wenku
  comment = "错词统计表"
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_word" {
    null    = true
    type    = character_varying(900)
    comment = "错词"
  }
  column "c_level" {
    null    = true
    type    = character_varying(100)
    comment = "错误级别"
  }
  column "c_errorcode" {
    null    = true
    type    = character_varying(100)
    comment = "错误类型"
  }
  column "c_message" {
    null    = true
    type    = character_varying(100)
    comment = "错误原因"
  }
  column "c_sentence" {
    null    = true
    type    = character_varying(900)
    comment = "所在句子"
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_tj_gwjd" {
  schema  = schema.db_wenku
  comment = "公文校对统计表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位id"
  }
  column "c_userid" {
    null    = true
    type    = character_varying(300)
    comment = "用户id"
  }
  column "c_ip" {
    null    = true
    type    = character_varying(300)
    comment = "IP"
  }
  column "n_cwsl" {
    null    = true
    type    = integer
    comment = "错误数量"
  }
  column "n_jgsl" {
    null    = true
    type    = integer
    comment = "警告数量"
  }
  column "n_tssl" {
    null    = true
    type    = integer
    comment = "提示数量"
  }
  column "n_jdzs" {
    null    = true
    type    = integer
    comment = "校对字数"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_nyr" {
    null    = true
    type    = character_varying(300)
    comment = "年月日，格式为：yyyy年MM月dd日"
  }
  column "c_ny" {
    null    = true
    type    = character_varying(300)
    comment = "年月，格式为：yyyy年MM月"
  }
  column "c_version" {
    null    = true
    type    = character_varying(300)
    comment = "客户端版本"
  }
  column "c_arch" {
    null    = true
    type    = character_varying(300)
    comment = "客户端架构"
  }
  column "c_ostype" {
    null    = true
    type    = character_varying(300)
    comment = "客户端操作系统"
  }
  column "c_osversion" {
    null    = true
    type    = character_varying(300)
    comment = "操作系统版本"
  }
  column "c_mac" {
    null    = true
    type    = character_varying(300)
    comment = "mac地址的md5"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_tj_gwpb" {
  schema  = schema.db_wenku
  comment = "公文排版统计表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位id"
  }
  column "c_userid" {
    null    = true
    type    = character_varying(300)
    comment = "用户id"
  }
  column "c_ip" {
    null    = true
    type    = character_varying(300)
    comment = "IP"
  }
  column "c_mbmc" {
    null    = true
    type    = character_varying(300)
    comment = "模版名称"
  }
  column "c_mblx" {
    null    = true
    type    = character_varying(300)
    comment = "模版类型"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_nyr" {
    null    = true
    type    = character_varying(300)
    comment = "年月日，格式为：yyyy年MM月dd日"
  }
  column "c_ny" {
    null    = true
    type    = character_varying(300)
    comment = "年月，格式为：yyyy年MM月"
  }
  column "c_version" {
    null    = true
    type    = character_varying(300)
    comment = "客户端版本"
  }
  column "c_arch" {
    null    = true
    type    = character_varying(300)
    comment = "客户端架构"
  }
  column "c_ostype" {
    null    = true
    type    = character_varying(300)
    comment = "客户端操作系统"
  }
  column "c_osversion" {
    null    = true
    type    = character_varying(300)
    comment = "操作系统版本"
  }
  column "c_mac" {
    null    = true
    type    = character_varying(300)
    comment = "mac地址的md5"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_tj_qt" {
  schema  = schema.db_wenku
  comment = "其他数据统计表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位id"
  }
  column "c_userid" {
    null = true
    type = character_varying(300)
  }
  column "c_ip" {
    null    = true
    type    = character_varying(300)
    comment = "IP"
  }
  column "n_type" {
    null    = true
    type    = integer
    comment = "业务类型，如语音朗读、公文比对"
  }
  column "n_data" {
    null    = true
    type    = bigint
    comment = "次数或字数等数据"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_nyr" {
    null    = true
    type    = character_varying(300)
    comment = "年月日，格式为：yyyy年MM月dd日"
  }
  column "c_ny" {
    null    = true
    type    = character_varying(300)
    comment = "年月，格式为：yyyy年MM月"
  }
  column "c_version" {
    null    = true
    type    = character_varying(300)
    comment = "客户端版本"
  }
  column "c_arch" {
    null    = true
    type    = character_varying(300)
    comment = "客户端架构"
  }
  column "c_ostype" {
    null    = true
    type    = character_varying(300)
    comment = "客户端操作系统"
  }
  column "c_osversion" {
    null    = true
    type    = character_varying(300)
    comment = "操作系统版本"
  }
  column "c_mac" {
    null    = true
    type    = character_varying(300)
    comment = "mac地址的md5"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_tj_syjl" {
  schema  = schema.db_wenku
  comment = "系统功能使用记录表（一个用户一天只保存一条记录）"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位id"
  }
  column "c_userid" {
    null    = true
    type    = character_varying(300)
    comment = "用户id"
  }
  column "c_ip" {
    null    = true
    type    = character_varying(300)
    comment = "IP"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_nyr" {
    null    = true
    type    = character_varying(300)
    comment = "年月日，格式为：yyyy年MM月dd日"
  }
  column "c_ny" {
    null    = true
    type    = character_varying(300)
    comment = "年月，格式为：yyyy年MM月"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_user_config" {
  schema  = schema.db_wenku
  comment = "意见反馈表"
  column "c_id" {
    null    = false
    type    = character_varying(300)
    comment = "编号/同用户编号"
  }
  column "c_config" {
    null    = true
    type    = text
    comment = "用户配置"
  }
  column "dt_cjsj" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "dt_xgsj" {
    null    = true
    type    = timestamp
    comment = "修改时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_user_data" {
  schema  = schema.db_wenku
  comment = "用户数据表"
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "主键"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "用户所在单位代码"
  }
  column "c_loginid" {
    null    = true
    type    = character_varying(300)
    comment = "用户登录标识"
  }
  column "c_key" {
    null    = true
    type    = character_varying(300)
    comment = "关键字"
  }
  column "c_value" {
    null    = true
    type    = text
    comment = "值"
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_user_feedback" {
  schema  = schema.db_wenku
  comment = "意见反馈表"
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_user_id" {
    null    = true
    type    = character_varying(300)
    comment = "用户编号"
  }
  column "c_user_nickname" {
    null    = true
    type    = character_varying(300)
    comment = "用户名称"
  }
  column "c_tenant_id" {
    null    = true
    type    = character_varying(300)
    comment = "租户编号"
  }
  column "c_tenant_name" {
    null    = true
    type    = character_varying(300)
    comment = "租户名称"
  }
  column "c_content" {
    null    = true
    type    = text
    comment = "反馈内容"
  }
  column "c_contact" {
    null    = true
    type    = character_varying(300)
    comment = "联系方式"
  }
  column "dt_cjsj" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "c_image_path" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_gwbj_xzsc" {
  schema  = schema.db_wenku
  comment = "写作素材表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_subject" {
    null    = false
    type    = character_varying(300)
    comment = "主题"
  }
  column "c_type" {
    null    = false
    type    = character_varying(100)
    comment = "类型，用典/金句/结构/用词/习近平讲话"
  }
  column "b_xjp" {
    null    = true
    type    = boolean
    comment = "是否习近平"
  }
  column "c_content" {
    null    = false
    type    = text
    comment = "内容"
  }
  column "c_source" {
    null    = true
    type    = character_varying(500)
    comment = "来源"
  }
  column "c_example" {
    null    = true
    type    = text
    comment = "例文"
  }
  column "c_allusion" {
    null    = true
    type    = text
    comment = "典故"
  }
  column "c_classify" {
    null    = true
    type    = character_varying(1000)
    comment = "类别"
  }
  column "c_explain" {
    null    = true
    type    = text
    comment = "释义"
  }
  column "c_appreciation" {
    null    = true
    type    = text
    comment = "赏析"
  }
  column "c_usage" {
    null    = true
    type    = character_varying(1000)
    comment = "用法"
  }
  column "c_fonttype" {
    null    = true
    type    = character_varying(300)
    comment = "字形"
  }
  column "dt_fbrq" {
    null    = true
    type    = timestamp
    comment = "发布日期"
  }
  column "n_frequency" {
    null    = true
    type    = integer
    comment = "使用次数"
  }
  column "dt_fwrq" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
  index "t_gwbj_xzsc_c_type_index" {
    columns = [column.c_type]
  }
}
table "t_label_map" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = bigserial
  }
  column "c_label_name" {
    null = false
    type = character_varying(100)
  }
  column "c_label_id" {
    null = false
    type = integer
  }
  column "c_placeholder" {
    null = true
    type = character_varying(200)
  }
  column "c_group_name" {
    null = true
    type = character_varying(50)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_learning_hub" {
  schema  = schema.db_wenku
  comment = "学习中心表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "主键ID"
  }
  column "c_type" {
    null    = true
    type    = character_varying(100)
    comment = "类型"
  }
  column "c_title" {
    null    = true
    type    = text
    comment = "标题"
  }
  column "c_content" {
    null    = true
    type    = text
    comment = "内容"
  }
  column "c_file_type" {
    null    = true
    type    = character_varying(20)
    comment = "文件后缀"
  }
  column "c_src_path" {
    null    = true
    type    = character_varying(255)
    comment = "原文件MinIO路径"
  }
  column "c_pdf_path" {
    null    = true
    type    = character_varying(255)
    comment = "PDF MinIO路径"
  }
  column "n_is_top" {
    null    = true
    type    = integer
    default = 0
    comment = "是否置顶：1置顶，0不置顶"
  }
  column "dt_create_time" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
    comment = "创建时间"
  }
  primary_key {
    columns = [column.c_id]
  }
  index "idx_learning_hub_create_time" {
    columns = [column.dt_create_time]
  }
  index "idx_learning_hub_type" {
    columns = [column.c_type]
  }
}
table "t_license" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_key" {
    null = true
    type = character_varying(50)
  }
  column "c_content" {
    null = true
    type = text
  }
  column "c_description" {
    null = true
    type = text
  }
  column "dt_time" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_llm_config" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_key" {
    null = true
    type = character_varying(300)
  }
  column "c_value" {
    null = true
    type = text
  }
  column "c_description" {
    null = true
    type = text
  }
  column "n_type" {
    null = true
    type = integer
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_lxxx" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_name" {
    null    = false
    type    = character_varying(300)
    comment = "姓名"
  }
  column "c_phone" {
    null    = false
    type    = character_varying(300)
    comment = "电话"
  }
  column "c_mail" {
    null    = false
    type    = character_varying(300)
    comment = "邮箱"
  }
  column "c_industry" {
    null    = true
    type    = character_varying(300)
    comment = "行业"
  }
  column "c_requirement" {
    null    = false
    type    = text
    comment = "业务需求"
  }
  column "c_company" {
    null    = true
    type    = character_varying(300)
    comment = "公司"
  }
  column "c_architecture" {
    null    = false
    type    = character_varying(100)
    comment = "架构"
  }
  column "c_submit_time" {
    null    = false
    type    = character_varying(100)
    comment = "提交时间"
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_t_lxxx_submit_time" {
    columns = [column.c_submit_time]
  }
}
table "t_notice" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_type" {
    null = true
    type = character_varying(100)
  }
  column "c_content" {
    null = true
    type = text
  }
  column "c_clientversion" {
    null = true
    type = character_varying(100)
  }
  column "n_valid" {
    null = true
    type = integer
  }
  column "dt_createtime" {
    null = true
    type = timestamp
  }
  column "c_corpid" {
    null = true
    type = character_varying(300)
  }
  column "dt_expiretime" {
    null = true
    type = timestamp
  }
  column "c_title" {
    null = true
    type = character_varying(300)
  }
  column "n_popup" {
    null = true
    type = integer
  }
  column "c_ext" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_notice_record" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_loginid" {
    null = true
    type = character_varying(100)
  }
  column "c_noticeid" {
    null = true
    type = character_varying(100)
  }
  column "c_corpid" {
    null = true
    type = character_varying(100)
  }
  column "n_valid" {
    null = true
    type = integer
  }
  column "n_rstatus" {
    null = true
    type = integer
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_notice_record_loginid" {
    columns = [column.c_loginid]
  }
}
table "t_open_lyzj_record" {
  schema  = schema.db_wenku
  comment = "第三方会议智记开放接口调用记录表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "任务ID，对外暴露的audioId"
  }
  column "c_file_name" {
    null    = true
    type    = character_varying(300)
    comment = "上传文件名"
  }
  column "n_state" {
    null    = true
    type    = integer
    comment = "状态：0处理中，1成功，2失败"
  }
  column "c_summary" {
    null    = true
    type    = text
    comment = "生成的结构化纪要JSON"
  }
  column "c_error_msg" {
    null    = true
    type    = character_varying(1000)
    comment = "失败原因"
  }
  column "c_client_ip" {
    null    = true
    type    = character_varying(300)
    comment = "调用方IP"
  }
  column "dt_create_time" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "dt_finish_time" {
    null    = true
    type    = timestamp
    comment = "完成时间"
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_open_lyzj_record_create_time" {
    columns = [column.dt_create_time]
  }
  index "i_open_lyzj_record_state" {
    columns = [column.n_state]
  }
}
table "t_operate_log" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(255)
  }
  column "dt_create_time" {
    null = true
    type = timestamp
  }
  column "c_error_message" {
    null = true
    type = text
  }
  column "c_file_size" {
    null = true
    type = character_varying(255)
  }
  column "c_ip" {
    null = true
    type = character_varying(255)
  }
  column "c_operation_content" {
    null = true
    type = text
  }
  column "c_operation_type" {
    null = true
    type = character_varying(255)
  }
  column "n_success" {
    null = true
    type = boolean
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_outer_corp" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "编号"
  }
  column "c_parentid" {
    null    = true
    type    = character_varying(300)
    comment = "上级机构id"
  }
  column "c_parentcode" {
    null    = true
    type    = character_varying(300)
    comment = "上级机构代码"
  }
  column "c_parentname" {
    null    = true
    type    = character_varying(300)
    comment = "上级机构名称"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位id"
  }
  column "c_corpcode" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_corpname" {
    null    = true
    type    = character_varying(300)
    comment = "单位名称"
  }
  column "c_lxr" {
    null    = true
    type    = character_varying(300)
    comment = "联系人"
  }
  column "dt_cjsj" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_outer_user" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "编号"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位id"
  }
  column "c_corpcode" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_userid" {
    null    = true
    type    = character_varying(300)
    comment = "用户id"
  }
  column "c_loginid" {
    null    = true
    type    = character_varying(300)
    comment = "用户登录标识"
  }
  column "c_nickname" {
    null = true
    type = character_varying(300)
  }
  column "dt_zcsj" {
    null    = true
    type    = timestamp
    comment = "注册时间"
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_ny" {
    null = true
    type = character_varying(100)
  }
  column "c_nyr" {
    null = true
    type = character_varying(100)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_print_config" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = bigserial
  }
  column "c_name" {
    null = true
    type = character_varying(256)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_qc_detail" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_taskid" {
    null = true
    type = character_varying(32)
  }
  column "c_loginid" {
    null = true
    type = character_varying(300)
  }
  column "c_filename" {
    null = true
    type = character_varying(300)
  }
  column "c_filepath" {
    null = true
    type = character_varying(300)
  }
  column "c_filecontent" {
    null = true
    type = text
  }
  column "c_title" {
    null = true
    type = character_varying(300)
  }
  column "c_fwzh" {
    null = true
    type = character_varying(300)
  }
  column "c_fwdw" {
    null = true
    type = character_varying(300)
  }
  column "c_errorresult" {
    null = true
    type = text
  }
  column "n_errorcount" {
    null = true
    type = integer
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_qc_detail_loginid" {
    columns = [column.c_loginid]
  }
  index "i_qc_detail_taskid" {
    columns = [column.c_taskid]
  }
}
table "t_qc_task" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_loginid" {
    null = true
    type = character_varying(300)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "dt_createtime" {
    null = true
    type = timestamp
  }
  column "n_filecount" {
    null = true
    type = integer
  }
  column "n_state" {
    null = true
    type = integer
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_qc_task_loginid" {
    columns = [column.c_loginid]
  }
  index "i_qc_task_name" {
    columns = [column.c_name]
  }
}
table "t_qr_code_config" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(64)
  }
  column "c_size_type" {
    null = true
    type = character_varying(32)
  }
  column "c_custom_width" {
    null = true
    type = numeric(10,2)
  }
  column "c_custom_height" {
    null = true
    type = numeric(10,2)
  }
  column "c_position_type" {
    null = true
    type = character_varying(32)
  }
  column "c_horizontal_value" {
    null = true
    type = numeric(10,2)
  }
  column "c_horizontal_type" {
    null = true
    type = character_varying(32)
  }
  column "c_vertical_value" {
    null = true
    type = numeric(10,2)
  }
  column "c_vertical_type" {
    null = true
    type = character_varying(32)
  }
  column "c_level1_name" {
    null = true
    type = character_varying(128)
  }
  column "c_level1_code" {
    null = true
    type = character_varying(64)
  }
  column "c_level2_name" {
    null = true
    type = character_varying(128)
  }
  column "c_level2_code" {
    null = true
    type = character_varying(64)
  }
  column "c_level3_name" {
    null = true
    type = character_varying(128)
  }
  column "c_level3_code" {
    null = true
    type = character_varying(64)
  }
  column "c_level4_name" {
    null = true
    type = character_varying(128)
  }
  column "c_level4_code" {
    null = true
    type = character_varying(64)
  }
  column "c_user_code" {
    null = true
    type = character_varying(64)
  }
  column "c_user_name" {
    null = true
    type = character_varying(64)
  }
  column "c_phone" {
    null = true
    type = character_varying(32)
  }
  column "c_user_id" {
    null = true
    type = character_varying(64)
  }
  column "c_token" {
    null = true
    type = character_varying(256)
  }
  column "dt_create" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  column "dt_update" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_qr_code_unit_list" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(64)
  }
  column "c_config_id" {
    null = false
    type = character_varying(64)
  }
  column "c_unit_type" {
    null = true
    type = character_varying(32)
  }
  column "c_unit_id" {
    null = true
    type = bigint
  }
  column "c_unit_name" {
    null = true
    type = character_varying(128)
  }
  column "n_order" {
    null = true
    type = integer
  }
  column "dt_create" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  column "dt_update" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_replaceword" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_key" {
    null    = true
    type    = character_varying(300)
    comment = "替换旧值"
  }
  column "c_value" {
    null    = true
    type    = character_varying(300)
    comment = "替换新值"
  }
  column "n_retain" {
    null    = true
    type    = integer
    default = 0
    comment = "是否保留"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_require" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_key" {
    null    = true
    type    = character_varying(300)
    comment = "写作类型"
  }
  column "c_content" {
    null    = true
    type    = text
    comment = "要求内容"
  }
  column "n_retain" {
    null    = true
    type    = integer
    default = 0
    comment = "是否保留"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_research_content" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(64)
  }
  column "c_task_id" {
    null = false
    type = character_varying(64)
  }
  column "c_status" {
    null = true
    type = character_varying(32)
  }
  column "c_user_input" {
    null = true
    type = text
  }
  column "dt_create_time" {
    null = true
    type = timestamp
  }
  column "dt_completed_time" {
    null = true
    type = timestamp
  }
  column "c_content" {
    null = true
    type = text
  }
  column "c_writer" {
    null = true
    type = text
  }
  column "c_extra_data" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_research_share" {
  schema = schema.db_wenku
  column "c_share_id" {
    null = false
    type = character_varying(64)
  }
  column "c_task_id" {
    null = false
    type = character_varying(150)
  }
  column "c_login_id" {
    null = false
    type = character_varying(64)
  }
  column "c_share_title" {
    null = true
    type = character_varying(500)
  }
  column "c_share_type" {
    null    = true
    type    = character_varying(32)
    default = "public"
  }
  column "c_password" {
    null = true
    type = character_varying(128)
  }
  column "c_expire_time" {
    null = true
    type = timestamp
  }
  column "c_access_count" {
    null    = true
    type    = integer
    default = 0
  }
  column "c_max_access_count" {
    null = true
    type = integer
  }
  column "c_status" {
    null    = true
    type    = character_varying(32)
    default = "active"
  }
  column "c_extra_data" {
    null = true
    type = text
  }
  column "dt_create_time" {
    null    = false
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  primary_key {
    columns = [column.c_share_id]
  }
}
table "t_research_tasks" {
  schema = schema.db_wenku
  column "c_task_id" {
    null = false
    type = character_varying(150)
  }
  column "c_login_id" {
    null = false
    type = character_varying(64)
  }
  column "c_status" {
    null    = false
    type    = character_varying(32)
    default = "进行中"
  }
  column "c_task_name" {
    null = true
    type = text
  }
  column "c_user_input" {
    null = true
    type = text
  }
  column "dt_create_time" {
    null    = false
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  column "c_extra_data" {
    null = true
    type = text
  }
  primary_key "pk_research_tasks" {
    columns = [column.c_task_id]
  }
}
table "t_role" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_type" {
    null    = true
    type    = character_varying(300)
    comment = "写作类型"
  }
  column "c_subtype" {
    null    = true
    type    = character_varying(300)
    comment = "子类型"
  }
  column "c_template" {
    null    = true
    type    = text
    comment = "提示词模板"
  }
  column "n_retain" {
    null    = true
    type    = integer
    default = 0
    comment = "是否保留"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_score_item" {
  schema = schema.db_wenku
  column "id" {
    null = false
    type = integer
  }
  column "pid" {
    null = false
    type = integer
  }
  column "c_type" {
    null = false
    type = character_varying(50)
  }
  column "c_item" {
    null = true
    type = text
  }
  column "c_score" {
    null = true
    type = text
  }
  column "c_desc" {
    null = true
    type = text
  }
  primary_key {
    columns = [column.id]
  }
  index "i_score_item_pid" {
    columns = [column.pid]
  }
}
table "t_sensitive_word" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_key" {
    null    = true
    type    = character_varying(300)
    comment = "敏感词"
  }
  column "c_value" {
    null    = true
    type    = character_varying(300)
    comment = "替换词"
  }
  column "n_retain" {
    null    = true
    type    = integer
    default = 0
    comment = "是否保留"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_session" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_sid" {
    null = true
    type = character_varying(300)
  }
  column "b_session" {
    null = true
    type = bytea
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_t_session" {
    columns = [column.c_sid]
  }
}
table "t_snpt_doctext" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_docid" {
    null    = false
    type    = character_varying(300)
    comment = "素材id"
  }
  column "c_textid" {
    null    = false
    type    = character_varying(300)
    comment = "文本id"
  }
  column "c_title" {
    null    = false
    type    = character_varying(300)
    comment = "文章标题"
  }
  column "c_text" {
    null    = false
    type    = text
    comment = "文章内容"
  }
  column "c_doctype" {
    null = true
    type = character_varying(50)
  }
  primary_key {
    columns = [column.c_id]
  }
  index "c_docid" {
    columns = [column.c_docid]
  }
  index "c_textid" {
    columns = [column.c_textid]
  }
}
table "t_snpt_favorite" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_docid" {
    null    = false
    type    = character_varying(300)
    comment = "素材id"
  }
  column "c_userid" {
    null    = false
    type    = character_varying(300)
    comment = "用户id"
  }
  column "c_textid" {
    null    = false
    type    = character_varying(300)
    comment = "内容id"
  }
  column "c_time" {
    null    = false
    type    = character_varying(100)
    comment = "创建时间"
  }
  column "c_doctype" {
    null = true
    type = character_varying(50)
  }
  primary_key {
    columns = [column.c_id]
  }
  index "c_userid" {
    columns = [column.c_userid]
  }
}
table "t_snpt_zsk_rel" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_zsk_id" {
    null = true
    type = character_varying(300)
  }
  column "c_zsk_code" {
    null = true
    type = character_varying(300)
  }
  column "c_rel_id" {
    null = true
    type = character_varying(300)
  }
  column "c_rel_type" {
    null = true
    type = character_varying(300)
  }
  primary_key "t_snpt_zsk_rel_key" {
    columns = [column.c_id]
  }
}
table "t_special_topic" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = true
    type = character_varying(100)
  }
  column "c_overview_image" {
    null = true
    type = text
  }
  column "c_overview_sm_image" {
    null = true
    type = text
  }
  column "c_rotation_images" {
    null = true
    type = text
  }
  column "c_description" {
    null = true
    type = text
  }
  column "n_enable" {
    null    = true
    type    = integer
    default = 0
  }
  column "dt_createtime" {
    null = true
    type = timestamp
  }
  column "c_plate" {
    null = true
    type = text
  }
  column "n_order" {
    null    = true
    type    = integer
    default = 1
  }
  primary_key "t_special_topic_pk" {
    columns = [column.c_id]
  }
}
table "t_sysconfig" {
  schema  = schema.db_wenku
  comment = "全局配置表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_name" {
    null    = true
    type    = character_varying(300)
    comment = "配置项名称"
  }
  column "c_key" {
    null    = true
    type    = character_varying(100)
    comment = "配置项关键字"
  }
  column "c_value" {
    null    = true
    type    = character_varying(300)
    comment = "显示项关键字"
  }
  column "n_order" {
    null    = true
    type    = integer
    comment = "显示顺序"
  }
  column "dt_createtime" {
    null    = true
    type    = timestamp
    comment = "创建时间"
  }
  column "dt_updatetime" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_template" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "编号"
  }
  column "c_type" {
    null    = true
    type    = character_varying(300)
    comment = "写作类型"
  }
  column "c_subtype" {
    null    = true
    type    = character_varying(300)
    comment = "子类型"
  }
  column "c_template" {
    null    = true
    type    = text
    comment = "提示词模板"
  }
  column "n_retain" {
    null    = true
    type    = integer
    default = 0
    comment = "是否保留"
  }
  column "c_title" {
    null = true
    type = character_varying(300)
  }
  column "dt_create" {
    null = true
    type = timestamp
  }
  column "dt_lastmodify" {
    null = true
    type = timestamp
  }
  column "n_rule" {
    null    = true
    type    = character_varying(20)
    default = "and"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_template_ban" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(40)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "c_file_path" {
    null = true
    type = character_varying(900)
  }
  column "c_create_time" {
    null = false
    type = bigint
  }
  column "n_order" {
    null    = true
    type    = integer
    default = 0
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_template_edit_config" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = bigserial
  }
  column "c_group_name" {
    null = true
    type = character_varying(20)
  }
  column "c_component_type" {
    null = true
    type = character_varying(50)
  }
  column "c_label_name" {
    null = true
    type = character_varying(20)
  }
  column "c_label_id" {
    null = true
    type = integer
  }
  column "c_row" {
    null = true
    type = integer
  }
  column "c_col" {
    null = true
    type = integer
  }
  column "c_item_order" {
    null = true
    type = integer
  }
  column "c_flex" {
    null = true
    type = character_varying(20)
  }
  column "c_is_show" {
    null    = true
    type    = boolean
    default = true
  }
  column "c_is_tick" {
    null    = true
    type    = boolean
    default = true
  }
  column "c_group_id" {
    null    = true
    type    = integer
    default = 1
  }
  column "c_gen_label_name" {
    null = true
    type = character_varying(25)
  }
  column "c_api_value" {
    null = true
    type = character_varying(50)
  }
  column "c_attr_value" {
    null = true
    type = character_varying(100)
  }
  column "c_placeholder" {
    null = true
    type = character_varying(20)
  }
  column "c_gen_attr_value" {
    null = true
    type = character_varying(30)
  }
  column "c_template_ban_id" {
    null = true
    type = character_varying(50)
  }
  column "c_bookmark" {
    null = true
    type = character_varying(10)
  }
  column "c_default_value" {
    null = true
    type = character_varying(100)
  }
  column "c_length" {
    null = true
    type = integer
  }
  column "c_add_content" {
    null = true
    type = character_varying(10)
  }
  column "c_add_position" {
    null = true
    type = character_varying(10)
  }
  column "c_align" {
    null = true
    type = character_varying(10)
  }
  column "c_indent" {
    null = true
    type = numeric(10)
  }
  column "c_font_style" {
    null = true
    type = character_varying(50)
  }
  column "c_font_color" {
    null = true
    type = character_varying(20)
  }
  column "c_font_size" {
    null = true
    type = character_varying(10)
  }
  column "c_border_top" {
    null = true
    type = character_varying(20)
  }
  column "c_border_bottom" {
    null = true
    type = character_varying(20)
  }
  column "c_left_indent" {
    null    = true
    type    = numeric(10,2)
    default = 0
  }
  column "c_right_indent" {
    null    = true
    type    = numeric(10,2)
    default = 0
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_template_edit_setting" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_config_key" {
    null = false
    type = character_varying(100)
  }
  column "c_config_value" {
    null = true
    type = character_varying(500)
  }
  column "c_config_type" {
    null = false
    type = character_varying(20)
  }
  column "c_description" {
    null = true
    type = character_varying(200)
  }
  column "c_create_time" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  column "c_update_time" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
  }
  primary_key {
    columns = [column.c_id]
  }
  unique "t_template_edit_setting_c_config_key_key" {
    columns = [column.c_config_key]
  }
}
table "t_template_value" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = bigserial
  }
  column "c_label_id" {
    null = false
    type = integer
  }
  column "c_attr_value" {
    null = true
    type = character_varying(500)
  }
  column "c_type" {
    null = true
    type = character_varying(50)
  }
  column "c_default_value" {
    null = true
    type = character_varying(2000)
  }
  column "c_gen_default_value" {
    null = true
    type = character_varying(100)
  }
  column "c_is_show" {
    null    = false
    type    = boolean
    default = true
  }
  column "c_is_tick" {
    null    = false
    type    = boolean
    default = true
  }
  column "c_is_gen_tick" {
    null    = false
    type    = boolean
    default = true
  }
  column "c_unit_default_value" {
    null = true
    type = character_varying(3)
  }
  column "c_template_id" {
    null = true
    type = character_varying(50)
  }
  column "c_ban_id" {
    null = true
    type = character_varying(50)
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_tj_gwjs" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_corpid" {
    null = true
    type = character_varying(300)
  }
  column "c_userid" {
    null = true
    type = character_varying(300)
  }
  column "c_ip" {
    null = true
    type = character_varying(300)
  }
  column "n_type" {
    null = true
    type = integer
  }
  column "dt_time" {
    null = true
    type = timestamp
  }
  column "c_nyr" {
    null = true
    type = character_varying(300)
  }
  column "c_ny" {
    null = true
    type = character_varying(300)
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_tj_gwjs_corpid" {
    columns = [column.c_corpid]
  }
}
table "t_tj_user_operate" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_userid" {
    null = true
    type = character_varying(300)
  }
  column "c_corpid" {
    null = true
    type = character_varying(300)
  }
  column "c_ip" {
    null = true
    type = character_varying(300)
  }
  column "n_type" {
    null = true
    type = integer
  }
  column "c_xznr" {
    null = true
    type = text
  }
  column "dt_time" {
    null = true
    type = timestamp
  }
  column "c_nyr" {
    null = true
    type = character_varying(100)
  }
  column "c_ny" {
    null = true
    type = character_varying(100)
  }
  primary_key {
    columns = [column.c_id]
  }
  index "t_user_operate_c_corpid_index" {
    columns = [column.c_corpid]
  }
  index "t_user_operate_c_userid_index" {
    columns = [column.c_userid]
  }
}
table "t_tj_wdgj" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_filename" {
    null = true
    type = character_varying(300)
  }
  column "c_userid" {
    null = true
    type = character_varying(300)
  }
  column "c_filepath" {
    null = true
    type = character_varying(300)
  }
  column "c_filelabel" {
    null = true
    type = character_varying(50)
  }
  column "c_filetype" {
    null = true
    type = character_varying(20)
  }
  column "c_abbrcontent" {
    null = true
    type = character_varying(600)
  }
  column "n_viewcount" {
    null = true
    type = integer
  }
  column "n_filelength" {
    null = true
    type = integer
  }
  column "n_wordcount" {
    null = true
    type = integer
  }
  column "n_storagetype" {
    null = true
    type = integer
  }
  column "n_filestatus" {
    null = true
    type = integer
  }
  column "dt_ctime" {
    null = true
    type = timestamp
  }
  column "dt_utime" {
    null = true
    type = timestamp
  }
  column "c_catagory" {
    null = true
    type = character_varying(300)
  }
  column "c_labels" {
    null = true
    type = character_varying(300)
  }
  column "n_encrypt" {
    null = true
    type = integer
  }
  column "c_dir_id" {
    null = true
    type = character_varying(50)
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_tj_wdgj_id" {
    columns = [column.c_id]
  }
}
table "t_tj_wdgj_dir" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = false
    type = character_varying(300)
  }
  column "c_userid" {
    null = false
    type = character_varying(300)
  }
  column "c_parent_id" {
    null = true
    type = character_varying(50)
  }
  column "dt_ctime" {
    null = true
    type = timestamp
  }
  column "dt_utime" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_tj_wdgj_dir_userid" {
    columns = [column.c_userid]
  }
}
table "t_tjfx" {
  schema  = schema.db_wenku
  comment = "统计分析表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_ip" {
    null    = true
    type    = character_varying(300)
    comment = "用户ip"
  }
  column "c_gn" {
    null    = true
    type    = character_varying(300)
    comment = "用户使用的功能(1 校对 0 排版 2 智能写作)"
  }
  column "c_mbmc" {
    null    = true
    type    = character_varying(300)
    comment = "模板名称"
  }
  column "c_mblx" {
    null    = true
    type    = character_varying(300)
    comment = "模板类型"
  }
  column "c_mj" {
    null    = true
    type    = character_varying(300)
    comment = "密级"
  }
  column "n_cwsl" {
    null    = true
    type    = integer
    comment = "错误数量"
  }
  column "n_jgsl" {
    null    = true
    type    = integer
    comment = "警告数量"
  }
  column "n_tssl" {
    null    = true
    type    = integer
    comment = "提示数量"
  }
  column "n_jdzs" {
    null = true
    type = integer
  }
  column "dt_gxsj" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_unit_person" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = bigint
    identity {
      generated = ALWAYS
    }
  }
  column "c_unit_person_id" {
    null = false
    type = character_varying(40)
  }
  column "c_unit_name" {
    null = false
    type = character_varying(100)
  }
  column "n_order" {
    null    = true
    type    = integer
    default = 0
  }
  column "c_create_time" {
    null = false
    type = bigint
  }
  column "c_remark" {
    null = true
    type = character_varying(200)
  }
  column "c_is_tick" {
    null = true
    type = boolean
  }
  column "c_group_id" {
    null = true
    type = bigint
  }
  column "c_common_group_id" {
    null = true
    type = bigint
  }
  primary_key {
    columns = [column.c_id]
  }
  index "i_unit_person_group_id" {
    columns = [column.c_group_id]
  }
  unique "uk_unit_person_id" {
    columns = [column.c_unit_person_id]
  }
}
table "t_unit_person_group" {
  schema = schema.db_wenku
  column "c_group_id" {
    null = false
    type = bigserial
  }
  column "c_group_name" {
    null = false
    type = character_varying(100)
  }
  column "n_order" {
    null    = true
    type    = integer
    default = 0
  }
  primary_key {
    columns = [column.c_group_id]
  }
  index "i_unit_person_group_order" {
    columns = [column.n_order]
  }
}
table "t_upfile" {
  schema = schema.db_wenku
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "主键"
  }
  column "c_name" {
    null    = true
    type    = character_varying(300)
    comment = "模板名称"
  }
  column "c_filename" {
    null    = true
    type    = character_varying(300)
    comment = "模板文件名"
  }
  column "c_type" {
    null    = true
    type    = character_varying(300)
    comment = "模版类型：default、import"
  }
  column "n_status" {
    null    = true
    type    = integer
    comment = "状态"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "c_groupid" {
    null    = true
    type    = character_varying(50)
    comment = "分组id"
  }
  column "n_order" {
    null    = true
    type    = integer
    comment = "显示顺序"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_user_login" {
  schema  = schema.db_wenku
  comment = "用户登录记录表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "单位代码"
  }
  column "c_corpname" {
    null = true
    type = character_varying(300)
  }
  column "c_userid" {
    null    = true
    type    = character_varying(300)
    comment = "用户登录标识"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "登录时间"
  }
  column "c_ny" {
    null = true
    type = character_varying(100)
  }
  column "c_nyr" {
    null = true
    type = character_varying(100)
  }
  column "c_ip" {
    null = true
    type = character_varying(300)
  }
  column "n_status" {
    null    = true
    type    = integer
    default = 1
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_usertemplate" {
  schema  = schema.db_wenku
  comment = "用户自定义模版表"
  column "c_id" {
    null    = false
    type    = character_varying(32)
    comment = "主键"
  }
  column "c_corpid" {
    null    = true
    type    = character_varying(300)
    comment = "用户所在单位代码"
  }
  column "c_loginid" {
    null    = true
    type    = character_varying(300)
    comment = "用户登录标识"
  }
  column "c_name" {
    null    = true
    type    = character_varying(300)
    comment = "模板名称"
  }
  column "c_filename" {
    null    = true
    type    = character_varying(300)
    comment = "模板文件名"
  }
  column "n_status" {
    null    = true
    type    = integer
    comment = "状态"
  }
  column "dt_time" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  column "n_order" {
    null    = true
    type    = integer
    comment = "显示顺序"
  }
  column "c_filepath" {
    null    = true
    type    = character_varying(900)
    comment = "模版文件路径"
  }
  column "c_jsonpath" {
    null    = true
    type    = character_varying(900)
    comment = "json文件路径"
  }
  column "c_imagepath" {
    null    = true
    type    = character_varying(900)
    comment = "缩略图文件路径"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_version" {
  schema  = schema.db_wenku
  comment = "版本信息表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "id"
  }
  column "c_modulename" {
    null    = true
    type    = character_varying(200)
    comment = "模块名称"
  }
  column "c_version" {
    null    = true
    type    = character_varying(200)
    comment = "版本号"
  }
  column "dt_updatetime" {
    null    = true
    type    = timestamp
    comment = "更新时间"
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_wdzs_wj" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(32)
  }
  column "c_corpid" {
    null = true
    type = character_varying(300)
  }
  column "c_loginid" {
    null = true
    type = character_varying(300)
  }
  column "c_username" {
    null = true
    type = character_varying(300)
  }
  column "c_bk" {
    null = true
    type = character_varying(50)
  }
  column "c_zt" {
    null = true
    type = character_varying(300)
  }
  column "n_xysh" {
    null = true
    type = integer
  }
  column "n_jzjkj" {
    null = true
    type = integer
  }
  column "c_wjm" {
    null = true
    type = character_varying(600)
  }
  column "c_wjlj" {
    null = true
    type = character_varying(900)
  }
  column "c_wjms" {
    null = true
    type = character_varying(900)
  }
  column "n_shzt" {
    null = true
    type = integer
  }
  column "dt_shsj" {
    null = true
    type = timestamp
  }
  column "c_shr" {
    null = true
    type = character_varying(300)
  }
  column "c_shbtgyy" {
    null = true
    type = character_varying(900)
  }
  column "n_drzt" {
    null = true
    type = integer
  }
  column "dt_drsj" {
    null = true
    type = timestamp
  }
  column "c_drjg" {
    null = true
    type = character_varying(300)
  }
  column "dt_cjsj" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_wdzsk_item" {
  schema  = schema.db_wenku
  comment = "文件和文件夹统一表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "ID"
  }
  column "c_knowledge_base_id" {
    null    = false
    type    = character_varying(50)
    comment = "所属知识库ID"
  }
  column "c_parent_id" {
    null    = true
    type    = character_varying(50)
    comment = "父文件夹ID，NULL表示在知识库根目录"
  }
  column "c_type" {
    null    = false
    type    = character_varying(50)
    comment = "类型：folder-文件夹，doc/pdf/xls/ppt等-文件类型"
  }
  column "c_name" {
    null    = false
    type    = character_varying(300)
    comment = "名称"
  }
  column "c_path" {
    null    = true
    type    = text
    comment = "路径（文件夹路径）"
  }
  column "n_level" {
    null    = true
    type    = integer
    default = 1
    comment = "层级，1为第一层"
  }
  column "n_file_count" {
    null    = true
    type    = integer
    default = 0
    comment = "文件数量"
  }
  column "c_file_path" {
    null    = true
    type    = character_varying(500)
    comment = "文件存储路径"
  }
  column "c_file_size" {
    null    = true
    type    = bigint
    comment = "文件大小（字节）"
  }
  column "n_word_count" {
    null    = true
    type    = integer
    default = 0
    comment = "字数"
  }
  column "c_es_index_id" {
    null    = true
    type    = character_varying(100)
    comment = "ES索引中的文档ID"
  }
  column "n_favorite" {
    null    = true
    type    = integer
    default = 0
    comment = "是否收藏：1-是，0-否"
  }
  column "c_minio_url" {
    null    = true
    type    = character_varying(500)
    comment = "MinIO对象存储URL"
  }
  column "c_create_user_id" {
    null    = true
    type    = character_varying(50)
    comment = "创建人ID"
  }
  column "c_update_user_id" {
    null    = true
    type    = character_varying(50)
    comment = "更新人ID"
  }
  column "dt_create_time" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
    comment = "创建时间"
  }
  column "dt_update_time" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
    comment = "更新时间"
  }
  column "n_status" {
    null    = true
    type    = integer
    default = 1
    comment = "状态：1-启用，0-禁用"
  }
  column "c_remark" {
    null    = true
    type    = text
    comment = "备注"
  }
  primary_key {
    columns = [column.c_id]
  }
  index "idx_item_create_time" {
    columns = [column.dt_create_time]
  }
  index "idx_item_es_index_id" {
    columns = [column.c_es_index_id]
  }
  index "idx_item_favorite" {
    columns = [column.n_favorite]
  }
  index "idx_item_kb_id" {
    columns = [column.c_knowledge_base_id]
  }
  index "idx_item_kb_parent" {
    columns = [column.c_knowledge_base_id, column.c_parent_id]
  }
  index "idx_item_kb_type" {
    columns = [column.c_knowledge_base_id, column.c_type]
  }
  index "idx_item_name" {
    columns = [column.c_name]
  }
  index "idx_item_parent_id" {
    columns = [column.c_parent_id]
  }
  index "idx_item_parent_type" {
    columns = [column.c_parent_id, column.c_type]
  }
  index "idx_item_status" {
    columns = [column.n_status]
  }
  index "idx_item_type" {
    columns = [column.c_type]
  }
}
table "t_wdzsk_knowledge_base" {
  schema  = schema.db_wenku
  comment = "知识库表"
  column "c_id" {
    null    = false
    type    = character_varying(50)
    comment = "知识库ID"
  }
  column "c_name" {
    null    = false
    type    = character_varying(300)
    comment = "知识库名称"
  }
  column "c_description" {
    null    = true
    type    = text
    comment = "知识库描述"
  }
  column "n_file_count" {
    null    = true
    type    = integer
    default = 0
    comment = "文件数量"
  }
  column "c_create_user_id" {
    null    = true
    type    = character_varying(50)
    comment = "创建人ID"
  }
  column "c_update_user_id" {
    null = true
    type = character_varying(50)
  }
  column "dt_create_time" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
    comment = "创建时间"
  }
  column "dt_update_time" {
    null    = true
    type    = timestamp
    default = sql("CURRENT_TIMESTAMP")
    comment = "更新时间"
  }
  column "n_status" {
    null    = true
    type    = integer
    default = 1
    comment = "状态：1-启用，0-禁用"
  }
  column "c_remark" {
    null    = true
    type    = text
    comment = "备注"
  }
  primary_key {
    columns = [column.c_id]
  }
  index "idx_kb_create_time" {
    columns = [column.dt_create_time]
  }
  index "idx_kb_create_user" {
    columns = [column.c_create_user_id]
  }
  index "idx_kb_name" {
    columns = [column.c_name]
  }
  index "idx_kb_status" {
    columns = [column.n_status]
  }
}
table "t_xmconfig" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "c_key" {
    null = true
    type = character_varying(100)
  }
  column "c_value" {
    null = true
    type = character_varying(300)
  }
  column "n_order" {
    null = true
    type = integer
  }
  column "dt_createtime" {
    null = true
    type = timestamp
  }
  column "dt_updatetime" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
}
table "t_xzsysconfig" {
  schema = schema.db_wenku
  column "c_id" {
    null = false
    type = character_varying(50)
  }
  column "c_group" {
    null = true
    type = character_varying(300)
  }
  column "c_name" {
    null = true
    type = character_varying(300)
  }
  column "c_key" {
    null = true
    type = character_varying(100)
  }
  column "c_value" {
    null = true
    type = character_varying(300)
  }
  column "n_order" {
    null = true
    type = integer
  }
  column "dt_create_time" {
    null = true
    type = timestamp
  }
  column "dt_update_time" {
    null = true
    type = timestamp
  }
  primary_key {
    columns = [column.c_id]
  }
}
schema "db_wenku" {
}
schema "public" {
  comment = "standard public schema"
}
