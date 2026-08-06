# リソースを作成する GCP プロジェクト ID。
variable "project_id" { type = string }

# Compute Engine リソースとスナップショットを配置するリージョン。
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

# VM とファイアウォールを配置する VPC ネットワーク。
variable "network" {
  type    = string
  default = "default"
}

# ゲームポートへの接続を許可する参加者の送信元 CIDR。
# 身内向け運用では固定 IP を /32 で指定し、全世界公開を避ける。
variable "game_source_ranges" {
  type    = list(string)
  default = ["0.0.0.0/0"]
}

# VM の起動・停止操作と DuckDNS Secret の読み取りを行う Cloud Run のサービスアカウント。
variable "cloud_run_service_account_email" { type = string }

# Budget Alert を作成する Billing Account ID。
variable "billing_account_id" { type = string }

# 予算通知の基準となる月額金額（JPY）。通知のみで、支出は自動停止しない。
variable "monthly_budget_jpy" {
  type    = number
  default = 5000
}

# Cloud Run に読み取りを許可する DuckDNS token の Secret ID。
variable "duckdns_secret_id" {
  type    = string
  default = "SEVEN_DAYS_DUCKDNS_TOKEN"
}
