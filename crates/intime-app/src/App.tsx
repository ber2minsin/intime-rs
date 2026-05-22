import "./styles/global.css";
import { ThemeProvider } from "@/components/theme-provider";
// import EmbeddingSearch from "@/components/embedding-search"
import Timeline from "@/components/timeline/timeline";

function App() {

  return (
    <ThemeProvider defaultTheme="dark" storageKey="vite-ui-theme">
      <div className="">
      {/* <EmbeddingSearch /> */}
      <Timeline />

      </div>
    </ThemeProvider>
  );
}

export default App;
