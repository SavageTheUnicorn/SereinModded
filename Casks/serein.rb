cask "serein" do
  version "1.0.0-nightly.7.1"
  sha256 "34aaebb5f6b4fd0b880498ff5f84733ac65fd6861c6c6a78ce4a837816bf6fc4"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
