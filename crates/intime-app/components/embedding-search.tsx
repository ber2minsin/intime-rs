import { useState } from "react";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { invoke } from "@tauri-apps/api/core";
import { Label } from "./ui/label";
import { ArrowUpIcon } from "lucide-react";

function EmbeddingSearch() {

    const [embeddingResponse, setEmbeddingResponse] = useState("");
    const [query, setQuery] = useState("");

    async function search() {
        // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
        setEmbeddingResponse(await invoke("search_embedding", { query }));
    }
    return (
        <div className="flex flex-col gap-3 w-full">
            <Label className="">Intime</Label>

            <form
                className="flex"
                onSubmit={(e) => {
                    e.preventDefault();
                    search();
                }}
            >
                <Input
                    id="search-input"
                    onChange={(e) => setQuery(e.currentTarget.value)}
                    placeholder="Search Activity"
                    className="max-w-64"
                />
                <Button type="submit" className="max-w-16"><ArrowUpIcon /></Button>
            </form>
            <p>{embeddingResponse}</p>
        </div>
    )
}

export default EmbeddingSearch;