// @ts-nocheck
// This file was generated by [tauri-specta](https://github.com/oscartbeaumont/tauri-specta). Do not edit this file manually.

/** user-defined commands **/


export const commands = {
async greet(name: string) : Promise<string> {
    return await TAURI_INVOKE("greet", { name });
},
async getConfig() : Promise<Config> {
    return await TAURI_INVOKE("get_config");
},
async saveConfig(config: Config) : Promise<Result<null, CommandError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("save_config", { config }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async login(username: string, password: string) : Promise<Result<LoginRespData, CommandError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("login", { username, password }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async getUserProfile() : Promise<Result<UserProfileRespData, CommandError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("get_user_profile") };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async search(keyword: string, pageNum: number) : Promise<Result<Pagination<ComicInSearchRespData>, CommandError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("search", { keyword, pageNum }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async getComic(pathWord: string) : Promise<Result<GetComicRespData, CommandError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("get_comic", { pathWord }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async getGroupChapters(comicPathWord: string, groupPathWord: string) : Promise<Result<ChapterInGetChaptersRespData[], CommandError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("get_group_chapters", { comicPathWord, groupPathWord }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async getChapter(comicPathWord: string, chapterUuid: string) : Promise<Result<GetChapterRespData, CommandError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("get_chapter", { comicPathWord, chapterUuid }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
}
}

/** user-defined events **/



/** user-defined constants **/



/** user-defined types **/

export type AuthorRespData = { name: string; alias: string | null; path_word: string }
export type ChapterInGetChapterRespData = { index: number; uuid: string; count: number; ordered: number; size: number; name: string; comic_id: string; comic_path_word: string; group_id: string | null; group_path_word: string; type: number; img_type: number; news: string; datetime_created: string; prev: string | null; next: string | null; contents: ContentRespData[]; words: number[]; is_long: boolean }
export type ChapterInGetChaptersRespData = { index: number; uuid: string; count: number; ordered: number; size: number; name: string; comic_id: string; comic_path_word: string; group_id: string | null; group_path_word: string; type: number; img_type: number; news: string; datetime_created: string; prev: string | null; next: string | null }
export type ComicInGetChapterRespData = { name: string; uuid: string; path_word: string; restrict: RestrictRespData }
export type ComicInGetComicRespData = { uuid: string; b_404: boolean; b_hidden: boolean; ban: number; ban_ip: boolean | null; name: string; alias: string | null; path_word: string; close_comment: boolean; close_roast: boolean; free_type: LabeledValueRespData; restrict: LabeledValueRespData; reclass: LabeledValueRespData; img_type: number; seo_baidu: string; region: LabeledValueRespData; status: LabeledValueRespData; author: AuthorRespData[]; theme: ThemeRespData[]; brief: string; datetime_updated: string; cover: string; last_chapter: LastChapterRespData; popular: number }
export type ComicInSearchRespData = { name: string; alias: string | null; path_word: string; cover: string; ban: number; img_type: number; author: AuthorRespData[]; popular: number }
export type CommandError = string
export type Config = { token: string; downloadDir: string }
export type ContentRespData = { url: string }
export type GetChapterRespData = { is_banned: boolean; show_app: boolean; is_lock: boolean; is_login: boolean; is_mobile_bind: boolean; is_vip: boolean; comic: ComicInGetChapterRespData; chapter: ChapterInGetChapterRespData }
export type GetComicRespData = { is_banned: boolean; is_lock: boolean; is_login: boolean; is_mobile_bind: boolean; is_vip: boolean; comic: ComicInGetComicRespData; popular: number; groups: { [key in string]: GroupRespData } }
export type GroupRespData = { path_word: string; count: number; name: string }
export type LabeledValueRespData = { value: number; display: string }
export type LastChapterRespData = { uuid: string; name: string }
export type LoginRespData = { token: string; user_id: string; username: string; nickname: string; avatar: string; datetime_created: string; ticket: number; reward_ticket: number; downloads: number; vip_downloads: number; reward_downloads: number; scy_answer: boolean }
export type Pagination<T> = { list: T[]; total: number; limit: number; offset: number }
export type RestrictRespData = { value: number; display: string }
export type ThemeRespData = { name: string; path_word: string }
export type UserProfileRespData = { user_id: string; username: string; nickname: string; avatar: string; datetime_created: string; ticket: number; reward_ticket: number; downloads: number; vip_downloads: number; reward_downloads: number; scy_answer: boolean; day_downloads_refresh: string; day_downloads: number }

/** tauri-specta globals **/

import {
	invoke as TAURI_INVOKE,
	Channel as TAURI_CHANNEL,
} from "@tauri-apps/api/core";
import * as TAURI_API_EVENT from "@tauri-apps/api/event";
import { type WebviewWindow as __WebviewWindow__ } from "@tauri-apps/api/webviewWindow";

type __EventObj__<T> = {
	listen: (
		cb: TAURI_API_EVENT.EventCallback<T>,
	) => ReturnType<typeof TAURI_API_EVENT.listen<T>>;
	once: (
		cb: TAURI_API_EVENT.EventCallback<T>,
	) => ReturnType<typeof TAURI_API_EVENT.once<T>>;
	emit: null extends T
		? (payload?: T) => ReturnType<typeof TAURI_API_EVENT.emit>
		: (payload: T) => ReturnType<typeof TAURI_API_EVENT.emit>;
};

export type Result<T, E> =
	| { status: "ok"; data: T }
	| { status: "error"; error: E };

function __makeEvents__<T extends Record<string, any>>(
	mappings: Record<keyof T, string>,
) {
	return new Proxy(
		{} as unknown as {
			[K in keyof T]: __EventObj__<T[K]> & {
				(handle: __WebviewWindow__): __EventObj__<T[K]>;
			};
		},
		{
			get: (_, event) => {
				const name = mappings[event as keyof T];

				return new Proxy((() => {}) as any, {
					apply: (_, __, [window]: [__WebviewWindow__]) => ({
						listen: (arg: any) => window.listen(name, arg),
						once: (arg: any) => window.once(name, arg),
						emit: (arg: any) => window.emit(name, arg),
					}),
					get: (_, command: keyof __EventObj__<any>) => {
						switch (command) {
							case "listen":
								return (arg: any) => TAURI_API_EVENT.listen(name, arg);
							case "once":
								return (arg: any) => TAURI_API_EVENT.once(name, arg);
							case "emit":
								return (arg: any) => TAURI_API_EVENT.emit(name, arg);
						}
					},
				});
			},
		},
	);
}
