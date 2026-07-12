variable "project_id" { type = string }
variable "region" {
  type = string
  default = "asia-northeast1"
}
variable "zone" {
  type = string
  default = "asia-northeast1-b"
}
variable "instance_name" {
  type = string
  default = "zunda-7dtd"
}
variable "machine_type" {
  type = string
  default = "e2-standard-4"
}
variable "network" {
  type = string
  default = "default"
}
variable "game_source_ranges" {
  type = list(string)
  default = ["0.0.0.0/0"]
}
variable "cloud_run_service_account_email" { type = string }
variable "billing_account_id" { type = string }
variable "monthly_budget_jpy" {
  type = number
  default = 5000
}
variable "duckdns_secret_id" {
  type = string
  default = "SEVEN_DAYS_DUCKDNS_TOKEN"
}
