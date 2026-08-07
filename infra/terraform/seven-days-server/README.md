# 7 Days to Die Terraform

State bucket は、この構成自身の state に依存させないため bootstrap で作成する。

```sh
gcloud storage buckets create gs://YOUR_TF_STATE_BUCKET --location=asia-northeast1 --uniform-bucket-level-access
gcloud storage buckets update gs://YOUR_TF_STATE_BUCKET --versioning
terraform init -backend-config="bucket=YOUR_TF_STATE_BUCKET" -backend-config="prefix=seven-days-server"
terraform plan -var-file=terraform.tfvars
terraform apply -var-file=terraform.tfvars
```

state bucket の IAM は Terraform 実行者だけに限定する。`terraform destroy` は data disk の `prevent_destroy` により失敗する設計であり、データディスクを明示的に state から外すなどの人手確認なしに解除しない。
