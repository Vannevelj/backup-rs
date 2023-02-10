# aws s3api list-objects --bucket zenzizenzi-photography --query 'Contents[].{Key: Key}' --output text >> objects.txt

BUCKET=zenzizenzi-photography
DAYS=14

while read x; do
  echo "Restore $x"
  aws s3api restore-object --bucket $BUCKET --key "$x" --restore-request Days=$DAYS,GlacierJobParameters={"Tier"="Bulk"};
done < objects.txt