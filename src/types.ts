import {AuthorRespData} from "./bindings.ts";

export interface ComicInfo {
    name: string;
    path_word: string;
    cover: string;
    author: AuthorRespData[];
}

export type CurrentTabName = "search" | "favorite" | "downloaded" | "chapter";