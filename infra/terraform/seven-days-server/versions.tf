terraform {
  # Terraform State は構成自身の外で作成した GCS バケットへ保存する。
  # 初回は README の backend-config を指定して terraform init を実行する。
  required_version = ">= 1.7.0"
  backend "gcs" {}
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
  }
}

# provider の project を変数で受け取り、region/zone の既定値を各リソースで共有する。
provider "google" {
  project               = var.project_id
  region                = var.region
  zone                  = var.zone
  user_project_override = true
  billing_project       = var.project_id
}
