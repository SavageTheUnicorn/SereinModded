cask "serein" do
  version "1.0.0-nightly.20260915.22"
  sha256 "d856732ea1ea837d0d9d3d0f5bff1e6cd9a44efcc2d860a3273263e93993bc87"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
