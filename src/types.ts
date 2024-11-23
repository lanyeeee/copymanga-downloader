import {AuthorRespData} from "./bindings.ts";

export type ComicInfo = {
    name: string;
    path_word: string;
    cover: string;
    author: AuthorRespData[];
}