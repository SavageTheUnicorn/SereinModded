cask "serein" do
  version "1.0.0-nightly.20260914.14"
  sha256 "2d4d64d914e3fbd4239db09feb1bf3c9dbaa334217c0d8254f678c922126e37a"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
