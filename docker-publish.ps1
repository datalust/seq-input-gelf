param (
  [Parameter(Mandatory=$true)]
  [string] $version
)

$versionParts = $version.Split('.')

$major = $versionParts[0]
$minor = $versionParts[1]

$baseImage = "datalust/sqelf-ci:$version"
$publishImages = "datalust/sqelf:latest", "datalust/sqelf:$major", "datalust/sqelf:$major.$minor", "datalust/sqelf:$version", "datalust/seq-input-gelf:latest", "datalust/seq-input-gelf:$major", "datalust/seq-input-gelf:$major.$minor", "datalust/seq-input-gelf:$version"

$choices  = "&Yes", "&No"
$decision = $Host.UI.PromptForChoice("Publishing ($baseImage) as ($publishImages)", "Does this look right?", $choices, 1)
if ($decision -eq 0) {
    foreach ($publishImage in $publishImages) {
        Write-Host "Publishing $publishImage"

        docker tag $baseImage $publishImage
        if ($LASTEXITCODE) { exit 1 }

        docker push $publishImage
        if ($LASTEXITCODE) { exit 1 }
    }
} else {
    Write-Host "Cancelled"
}
