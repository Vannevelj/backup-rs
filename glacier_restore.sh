BUCKET=zenzizenzi-photography
DAYS=14

for x in `aws s3 ls s3://$BUCKET --recursive  | awk '{print $4}'`;
 do
  echo "1:Restore $x"
  #aws s3api restore-object --bucket $BUCKET --key $x --restore-request Days=$DAYS,GlacierJobParameters={"Tier"="Standard"};
done