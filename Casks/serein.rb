cask "serein" do
  version "1.0.0-nightly.20260914.16"
  sha256 "2ce5247e3e0c42bf9ce5a856d70cca14544e3efde5ab3a08d05783e3683752ce"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
