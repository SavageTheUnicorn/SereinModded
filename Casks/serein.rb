cask "serein" do
  version "1.0.0-nightly.20260916.27"
  sha256 "9eccf31111b355999ab322261e8668f09f4d848d48f52fe160e2e32d2806ce8d"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
