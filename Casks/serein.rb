cask "serein" do
  version "1.0.0-nightly.20260916.31"
  sha256 "a09f7d28e0f74183beccabcf9508f5ba112edc6684654cd3eed3ffbdd82ea469"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
