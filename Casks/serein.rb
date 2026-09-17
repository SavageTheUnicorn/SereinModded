cask "serein" do
  arch arm: "ARM64", intel: "X64"

  version "1.0.0-nightly.20260916.34"
  sha256 arm:   "8b38f14ae2dd7eb8b4ccc90a1d8f76a77da8cabcdb7463e4c90b24c8f34b0f82",
         intel: "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-#{arch}.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on macos: :sonoma

  app "Serein.app"
end
