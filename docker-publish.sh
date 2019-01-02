# call as ./docker-publish 1.0.0

set -e

version=$1

IFS='.' read -ra parts <<< "$version"
major="${parts[0]}"

echo "Pushing:
  datalust/sqelf:latest
  datalust/sqelf:$version
  datalust/sqelf:$major

Based off:
  datalust/sqelf-ci:$version
"

echo "Push these public container images?"
select yn in "Yes" "No"; do
    case $yn in
        Yes ) break;;
        No ) exit;;
    esac
done

echo "Pushing datalust/sqelf"

docker pull datalust/sqelf-ci:$version
docker tag datalust/sqelf-ci:$version datalust/sqelf:latest
docker tag datalust/sqelf-ci:$version datalust/sqelf:$version
docker tag datalust/sqelf-ci:$version datalust/sqelf:$major

docker push datalust/sqelf:latest
docker push datalust/sqelf:$version
docker push datalust/sqelf:$major

echo "Done!"
