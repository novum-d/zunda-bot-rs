# リソースを作成する GCP プロジェクト ID。
variable "project_id" { type = string }

# Compute Engine リソースと GCS バックアップを配置するリージョン。
variable "region" {
  type    = string
  default = "asia-northeast1"
}

# VM とゾーンディスクを配置するゾーン。region 配下の zone を指定する。
variable "zone" {
  type    = string
  default = "asia-northeast1-b"
}

# VM、ディスク、ファイアウォールの名前に使う識別子。
variable "instance_name" {
  type    = string
  default = "zunda-7dtd"
}

# 7DTD サーバー VM のマシンタイプ。
variable "machine_type" {
  type    = string
  default = "e2-standard-4"
}

# セーブデータ、ワールド、Mod を置くデータディスクの容量（GiB）。
# テスト用の初期値は 10 GiB とする。継続運用では 20 GiB 以上を推奨する。
variable "data_disk_size_gb" {
  type    = number
  default = 10
}

# バックアップ用 GCS バケット名。空の場合はプロジェクト ID から生成する。
variable "backup_bucket_name" {
  type    = string
  default = ""
}

# GCS バックアップの保持日数。期限を過ぎたオブジェクトは自動削除される。
variable "backup_retention_days" {
  type    = number
  default = 30
}

# Cloud BillingのStandard usage cost exportを保存するBigQuery dataset。
variable "billing_export_dataset_id" {
  type    = string
  default = "billing_export"
}

# 初回有効化時に前月からのbackfillを利用できるUS multi-regionを既定とする。
variable "billing_export_location" {
  type    = string
  default = "US"
}

# VM とファイアウォールを配置する VPC ネットワーク。
variable "network" {
  type    = string
  default = "default"
}

# ゲームポートへの接続を許可する参加者の送信元 CIDR。
# 身内向け運用では固定 IP を /32 で指定し、全世界公開を避ける。
variable "game_source_ranges" {
  type = list(string)

  validation {
    condition = length(var.game_source_ranges) > 0 && alltrue([
      for cidr in var.game_source_ranges : can(cidrnetmask(cidr)) && endswith(cidr, "/32")
    ])
    error_message = "game_source_ranges には接続を許可するIPv4アドレスを /32 形式で1件以上指定してください。"
  }
}

# VM の起動・停止操作と DuckDNS Secret の読み取りを行う Cloud Run のサービスアカウント。
variable "cloud_run_service_account_email" { type = string }

# Budget Alert を作成する Billing Account ID。
variable "billing_account_id" { type = string }

# 予算通知の基準となる月額金額（JPY）。通知のみで、支出は自動停止しない。
variable "monthly_budget_jpy" {
  type    = number
  default = 2000
}

# Cloud Run に読み取りを許可する DuckDNS token の Secret ID。
variable "duckdns_secret_id" {
  type    = string
  default = "SEVEN_DAYS_DUCKDNS_TOKEN"
}
