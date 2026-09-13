cask "serein" do
  version "1.0.0-nightly.10.1"
  sha256 "4d80901109e97337e1266c84998b48128e01db98040509fb114626b629064ff6"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
